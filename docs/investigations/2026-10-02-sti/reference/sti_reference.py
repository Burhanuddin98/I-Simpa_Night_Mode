"""Independent reference implementation of the STI, IEC 60268-16:2011 (edition 4).

Written from the standard's text only (read as BS EN 60268-16:2011, identical to IEC 60268-16:2011),
for testing another implementation. No code from this repository or from upstream I-Simpa was read.

Clauses implemented
-------------------
- 6.1        MTF from the impulse response (Schroeder): m_k(F) = |int h^2 e^{-j2piFt} dt| / int h^2 dt.
- 8.3        Prediction from a simulated IR: thresholds and noise into the MTF, masking (Table A.1),
             male and female weights (Table A.3). Duration >= 1.6 s and >= T/2 (8.3 a, 6.2 b).
- A.3.2      Level-dependent auditory masking, Table A.1. Band k is masked by band k-1's TOTAL level
             (signal + noise, A.3.2 last paragraph and NOTE 1). 125 Hz is not masked.
- A.3.3      Absolute speech reception threshold, Table A.2.
- A.3.4      Weighting (alpha) and redundancy (beta) factors, Table A.3.
- A.3.5      Speech spectra re the A-weighted speech level, Table A.4.
- A.5.3      m' = m * I_k / (I_k + I_am,k + I_rt,k [+ I_n,k]) ; m' > 1 truncated to 1 (NOTE 1, NOTE 2).
- A.5.4      SNR_eff = 10 lg(m'/(1-m')), limited to [-15, +15] dB.
- A.5.5      TI = (SNR_eff + 15)/30.
- A.5.6      MTI_k = mean over the 14 modulation frequencies;
             STI = sum alpha_k MTI_k - sum beta_k sqrt(MTI_k MTI_k+1); STI > 1 set to 1 (NOTE).
- J.3        Test speech level 60 dB(A) (70 dB(A) for raised vocal effort).

Readings taken where the text needs one
---------------------------------------
1. Cl. 6.1 prints the numerator as |int h_k(t) e^{-j2pi f t} dt| (h, not h^2) over a denominator of
   int h_k(t)^2 dt. That is dimensionally inconsistent and contradicts A.1.2 (the MTF is defined on the
   intensity envelope; exponential decay gives 1/sqrt(1+(2piFT/13.8)^2), which only the h^2 form yields).
   This module takes the energy (h^2) in both integrals; its input is already an energy IR.
2. The 6.1 noise factor [1 + 10^(-SNR_k/10)]^-1 and A.5.3 NOTE 2's I_n,k in the denominator are the same
   correction; it is applied once, through A.5.3 (m_ir is returned noise-free, m_noise separately).
   Annex M's worked example factors it the same way (noise-only factor x masking/threshold factor whose I_k is
   the combined signal+noise intensity), which is algebraically identical.
3. Table A.1 is applied exactly as printed, including its step at L = 100 dB (0.5*100 - 59.8 = -9.8 dB just
   below, -10 dB at and above), although A.3.2 NOTE 2 calls the scheme continuous.
4. Time bins: bin i is placed at t_i = i*dt from the direct sound. |sum E_i e^{-j2piF t_i}| is invariant under a
   uniform time shift, so the choice of bin edge vs centre does not change m.
"""

from __future__ import annotations

import math
from typing import Sequence

import numpy as np

# --- Constants from the standard -------------------------------------------------------------------------

BANDS_HZ = (125, 250, 500, 1000, 2000, 4000, 8000)

# A.2.2: 14 modulation frequencies, one-third-octave spaced
MOD_FREQS_HZ = (0.63, 0.80, 1.00, 1.25, 1.60, 2.00, 2.50, 3.15, 4.00, 5.00, 6.3, 8.00, 10.0, 12.5)

# Table A.2: absolute speech reception threshold, dB SPL
ART_DB = (46.0, 27.0, 12.0, 6.5, 7.5, 8.0, 12.0)

# Table A.3: weighting factors alpha_k and redundancy factors beta_k (beta_k between band k and k+1).
# Female has no 125 Hz band: represented by 0.0 weight (the band does not enter the female STI).
ALPHA = {
    "male": (0.085, 0.127, 0.230, 0.233, 0.309, 0.224, 0.173),
    "female": (0.0, 0.117, 0.223, 0.216, 0.328, 0.250, 0.194),
}
BETA = {
    "male": (0.085, 0.078, 0.065, 0.011, 0.047, 0.095),
    "female": (0.0, 0.099, 0.066, 0.062, 0.025, 0.076),
}

# Table A.4: octave band levels, dB re the A-weighted speech level. Female 125 Hz is "-" (no band).
SPEECH_SPECTRUM_DB = {
    "male": (2.9, 2.9, -0.8, -6.8, -12.8, -18.8, -24.8),
    "female": (float("nan"), 5.3, -1.9, -9.1, -15.8, -16.7, -18.0),
}

DEFAULT_SPEECH_LEVEL_DBA = 60.0  # J.3: 60 dB(A) at 1 m on axis; 70 dB(A) for raised vocal effort
MIN_IR_DURATION_S = 1.6  # 6.2 b, 8.3 a
SNR_LIMIT_DB = 15.0  # A.5.4


# --- Helpers ---------------------------------------------------------------------------------------------

def speech_spectrum(sex: str = "male", level_dba: float = DEFAULT_SPEECH_LEVEL_DBA) -> np.ndarray:
    """Table A.4: octave-band speech levels (dB SPL) for an A-weighted speech level `level_dba`.

    Returns 7 values, 125..8000 Hz. Female 125 Hz is NaN (the table has no value: "-").
    """
    return np.asarray(SPEECH_SPECTRUM_DB[sex], dtype=float) + float(level_dba)


def db_to_intensity(level_db) -> np.ndarray:
    """I = 10^(L/10) (A.3.2, A.3.3). NaN or -inf level -> 0 intensity."""
    L = np.asarray(level_db, dtype=float)
    with np.errstate(over="ignore", invalid="ignore"):
        out = np.where(np.isfinite(L), 10.0 ** (L / 10.0), 0.0)
    return out


def masking_db(level_prev_db: float) -> float:
    """Table A.1: auditory masking amdB (dB) for band k, from band k-1's total level L (dB)."""
    L = float(level_prev_db)
    if L < 63.0:
        return 0.5 * L - 65.0
    if L < 67.0:
        return 1.8 * L - 146.9
    if L < 100.0:
        return 0.5 * L - 59.8
    return -10.0


def masking_factor(level_prev_db: float) -> float:
    """A.3.2: amf = 10^(amdB/10)."""
    return 10.0 ** (masking_db(level_prev_db) / 10.0)


def reception_threshold_intensity() -> np.ndarray:
    """A.3.3: I_rt,k = 10^(ART_k/10), 7 bands."""
    return 10.0 ** (np.asarray(ART_DB) / 10.0)


# --- Clause 6.1: MTF from the energy impulse response ----------------------------------------------------

def mtf_from_energy_ir(energy: Sequence[float], dt: float,
                       mod_freqs: Sequence[float] = MOD_FREQS_HZ) -> np.ndarray:
    """Cl. 6.1 first factor (Schroeder), energy form: m(F) = |sum E_i e^{-j2piF i dt}| / sum E_i.

    `energy` is the energy per time bin (h^2 integrated over the bin), bin 0 starting at the direct sound.
    Returns len(mod_freqs) values. Noise-free: the SNR factor is applied in `correct_mtf`.
    """
    E = np.asarray(energy, dtype=float)
    if E.ndim != 1 or E.size == 0:
        raise ValueError("energy must be a non-empty 1-D array")
    if dt <= 0:
        raise ValueError("dt must be > 0")
    total = E.sum()
    if not total > 0:
        raise ValueError("energy IR has no energy")
    t = np.arange(E.size) * dt
    F = np.asarray(mod_freqs, dtype=float)
    phase = np.exp(-2j * np.pi * np.outer(F, t))
    return np.abs(phase @ E) / total


# --- A.5.3: masking, threshold and noise correction ------------------------------------------------------

def correct_mtf(m: np.ndarray, L_s: Sequence[float] | None, L_n: Sequence[float] | None = None,
                auditory_effects: bool = True) -> dict:
    """A.5.3 with A.3.2 / A.3.3 and NOTE 2 (ambient noise): m' = m * I_s / (I_s + I_n + I_am + I_rt).

    m        : (7, n_F) noise-free MTF (rows 125..8000 Hz).
    L_s, L_n : 7 octave-band signal and noise levels, dB SPL at the listener. L_n None = no noise.
               L_s may be None only if auditory_effects is False and L_n is None (level-free case).
    auditory_effects False disables masking and threshold (A.3.1 NOTE: electrical signals).

    I_am,k = I_tot,k-1 * amf(L_tot,k-1), L_tot = signal + noise (A.3.2 last paragraph, NOTE 1). I_am,125 = 0.
    m' > 1 is truncated to 1 (A.5.3 NOTE 1).
    """
    m = np.asarray(m, dtype=float)
    nb = m.shape[0]
    if L_s is None:
        if auditory_effects or L_n is not None:
            raise ValueError("L_s is required when auditory effects or noise are applied")
        factor = np.ones(nb)
        mc = np.minimum(m, 1.0)
        return {"m": m, "m_corrected": mc, "factor": factor, "I_s": None, "I_n": None, "L_total": None,
                "amdB": None, "amf": None, "I_am": np.zeros(nb), "I_rt": np.zeros(nb),
                "m_noise": np.ones(nb), "m_masking_threshold": np.ones(nb)}

    I_s = db_to_intensity(L_s)
    I_n = db_to_intensity(L_n) if L_n is not None else np.zeros(nb)
    I_tot = I_s + I_n
    with np.errstate(divide="ignore"):
        L_total = np.where(I_tot > 0, 10.0 * np.log10(np.where(I_tot > 0, I_tot, 1.0)), -np.inf)

    amdB = np.full(nb, np.nan)
    amf = np.zeros(nb)
    I_am = np.zeros(nb)
    if auditory_effects:
        for k in range(1, nb):
            if I_tot[k - 1] > 0:
                amdB[k] = masking_db(L_total[k - 1])
                amf[k] = 10.0 ** (amdB[k] / 10.0)
                I_am[k] = I_tot[k - 1] * amf[k]
        I_rt = reception_threshold_intensity()[:nb]
    else:
        I_rt = np.zeros(nb)

    denom = I_s + I_n + I_am + I_rt
    factor = np.where(denom > 0, I_s / np.where(denom > 0, denom, 1.0), 0.0)
    # Annex M style split: noise-only factor x masking/threshold factor (identical product).
    m_noise = np.where(I_tot > 0, I_s / np.where(I_tot > 0, I_tot, 1.0), 0.0)
    den2 = I_tot + I_am + I_rt
    m_mt = np.where(den2 > 0, I_tot / np.where(den2 > 0, den2, 1.0), 0.0)

    mc = np.minimum(m * factor[:, None], 1.0)
    return {"m": m, "m_corrected": mc, "factor": factor, "I_s": I_s, "I_n": I_n, "L_total": L_total,
            "amdB": amdB, "amf": amf, "I_am": I_am, "I_rt": I_rt,
            "m_noise": m_noise, "m_masking_threshold": m_mt}


# --- A.5.4 - A.5.6 ---------------------------------------------------------------------------------------

def snr_eff(m_corrected: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """A.5.4: SNR_eff = 10 lg(m'/(1-m')). Returns (unclipped, clipped to [-15, +15] dB)."""
    mc = np.clip(np.asarray(m_corrected, dtype=float), 0.0, 1.0)
    with np.errstate(divide="ignore", invalid="ignore"):
        raw = 10.0 * np.log10(mc / (1.0 - mc))
    raw = np.where(mc >= 1.0, np.inf, np.where(mc <= 0.0, -np.inf, raw))
    return raw, np.clip(raw, -SNR_LIMIT_DB, SNR_LIMIT_DB)


def transmission_index(snr_clipped: np.ndarray) -> np.ndarray:
    """A.5.5: TI = (SNR_eff + 15)/30."""
    return (np.asarray(snr_clipped, dtype=float) + 15.0) / 30.0


def modulation_transfer_index(ti: np.ndarray) -> np.ndarray:
    """A.5.6: MTI_k = (1/n) sum_m TI_k,fm (mean across modulation frequencies, axis 1)."""
    return np.asarray(ti, dtype=float).mean(axis=1)


def sti_from_mti(mti: Sequence[float], sex: str = "male") -> dict:
    """A.5.6 with Table A.3: STI = sum alpha_k MTI_k - sum beta_k sqrt(MTI_k MTI_k+1); STI > 1 -> 1.

    Female: the 125 Hz band carries no weight (alpha and beta are '-' in Table A.3).
    Returns {'sti': truncated, 'sti_raw', 'sum_alpha_mti', 'sum_beta_mti'}.
    """
    x = np.asarray(mti, dtype=float)
    a = np.asarray(ALPHA[sex])
    b = np.asarray(BETA[sex])
    if sex == "female":  # do not let a NaN 125 Hz MTI poison a zero weight
        x = x.copy()
        x[0] = 0.0
    sa = float(np.sum(a * x))
    sb = float(np.sum(b * np.sqrt(x[:-1] * x[1:])))
    raw = sa - sb
    return {"sti": min(raw, 1.0), "sti_raw": raw, "sum_alpha_mti": sa, "sum_beta_mti": sb}


def sti_from_mtf(m: np.ndarray, L_s: Sequence[float] | None, L_n: Sequence[float] | None = None,
                 auditory_effects: bool = True) -> dict:
    """Full chain from a noise-free (7, 14) MTF matrix: A.5.3 -> A.5.6, both sexes.

    The same m' feeds both weightings. To use each sex's own speech spectrum, call once per sex with
    L_s = speech_spectrum(sex, level) and read that sex's result.
    """
    corr = correct_mtf(m, L_s, L_n, auditory_effects)
    snr_raw, snr = snr_eff(corr["m_corrected"])
    ti = transmission_index(snr)
    mti = modulation_transfer_index(ti)
    out = dict(corr)
    out.update({"snr_eff_raw": snr_raw, "snr_eff": snr, "ti": ti, "mti": mti,
                "male": sti_from_mti(mti, "male"), "female": sti_from_mti(mti, "female")})
    out["sti_male"] = out["male"]["sti"]
    out["sti_female"] = out["female"]["sti"]
    return out


def sti_reference(energy_irs: Sequence[Sequence[float]], dt: float, L_s: Sequence[float] | None,
                  L_n: Sequence[float] | None = None, auditory_effects: bool = True,
                  reverberation_time_s: float | None = None) -> dict:
    """STI from 7 octave-band energy IRs (125..8000 Hz), cl. 6.1 + 8.3 + Annex A.

    energy_irs : 7 arrays of energy per time bin (bin 0 at the direct sound); lengths may differ.
    dt         : bin width, s.
    L_s, L_n   : 7 signal / noise levels at the listener, dB SPL (L_n None = noise-free).
    reverberation_time_s : optional T, for the 8.3 a check duration >= T/2.

    Returns every intermediate: m_ir (7x14, noise-free), m_noise, amdB, amf, I_am, I_rt, factor,
    m_corrected (7x14), snr_eff_raw, snr_eff (clipped), ti, mti, male/female dicts, sti_male, sti_female,
    durations and the 8.3 a duration flags.
    """
    if len(energy_irs) != len(BANDS_HZ):
        raise ValueError("need 7 octave bands, 125..8000 Hz")
    m_ir = np.vstack([mtf_from_energy_ir(e, dt) for e in energy_irs])
    out = sti_from_mtf(m_ir, L_s, L_n, auditory_effects)
    out["m_ir"] = m_ir
    durations = np.array([len(e) * dt for e in energy_irs])
    out["durations_s"] = durations
    out["duration_ok_1_6s"] = bool(np.all(durations >= MIN_IR_DURATION_S))
    out["duration_ok_half_T"] = (None if reverberation_time_s is None
                                 else bool(np.all(durations >= 0.5 * reverberation_time_s)))
    return out


def exponential_decay_mtf(F, T: float) -> np.ndarray:
    """A.1.2, Figure A.2 case A: m(F) = 1/sqrt(1 + (2 pi F T / 13.8)^2)."""
    F = np.asarray(F, dtype=float)
    return 1.0 / np.sqrt(1.0 + (2.0 * math.pi * F * T / 13.8) ** 2)


def noise_mtf(snr_db) -> np.ndarray:
    """A.1.2, Figure A.2 case B: m = 1/(1 + 10^(-SNR/10))."""
    return 1.0 / (1.0 + 10.0 ** (-np.asarray(snr_db, dtype=float) / 10.0))
