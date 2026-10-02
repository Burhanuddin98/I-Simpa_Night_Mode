"""Tests for sti_reference.py. Every expected value is hand-derived from IEC 60268-16:2011 (edition 4);
the clause is named in each test. Annex M numbers are the standard's printed worked example (Table M.1).

Run: python -m pytest -q docs/investigations/2026-10-02-sti/reference
"""

import math
import os
import sys

import numpy as np
import pytest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sti_reference as s  # noqa: E402

NF = len(s.MOD_FREQS_HZ)


def delta_irs():
    return [np.array([1.0]) for _ in s.BANDS_HZ]


# --- A.2.2 / Tables A.2-A.4 constants --------------------------------------------------------------------

def test_modulation_frequencies_A22():
    """A.2.2: 14 one-third-octave modulation frequencies 0.63 .. 12.5 Hz."""
    assert s.MOD_FREQS_HZ == (0.63, 0.8, 1.0, 1.25, 1.6, 2.0, 2.5, 3.15, 4.0, 5.0, 6.3, 8.0, 10.0, 12.5)


def test_weights_sum_to_one_table_A3():
    """Table A.3: sum(alpha) - sum(beta) = 1.000 for both sexes (male 1.381-0.381, female 1.328-0.328)."""
    assert sum(s.ALPHA["male"]) == pytest.approx(1.381, abs=1e-12)
    assert sum(s.BETA["male"]) == pytest.approx(0.381, abs=1e-12)
    assert sum(s.ALPHA["female"]) == pytest.approx(1.328, abs=1e-12)
    assert sum(s.BETA["female"]) == pytest.approx(0.328, abs=1e-12)


def test_reception_threshold_table_A2():
    """Table A.2 ART (dB SPL) and A.3.3 I_rt,k = 10^(ART_k/10)."""
    assert s.ART_DB == (46, 27, 12, 6.5, 7.5, 8, 12)
    np.testing.assert_allclose(s.reception_threshold_intensity(),
                               [10 ** 4.6, 10 ** 2.7, 10 ** 1.2, 10 ** 0.65, 10 ** 0.75, 10 ** 0.8, 10 ** 1.2])


def test_reception_threshold_halves_m_at_ART_125():
    """A.3.3 + A.5.3: 125 Hz is never masked (A.3.2), so with L_s,125 = ART_125 = 46 dB and no noise,
    m' = m * I/(I + I_rt) = m * 0.5 exactly."""
    Ls = [46.0, 0, 0, 0, 0, 0, 0]
    c = s.correct_mtf(np.ones((7, NF)), Ls)
    assert c["I_am"][0] == 0.0
    assert c["factor"][0] == pytest.approx(0.5, abs=1e-12)
    assert np.all(c["m_corrected"][0] == pytest.approx(0.5, abs=1e-12))


def test_reception_threshold_each_band():
    """A.3.3: at L_s,k = ART_k with masking negligible, m' = 0.5 in each band. Levels alternate so that band
    k-1 is quiet (masking by a 0 dB SPL band: 0.5*0-65 = -65 dB -> I_am = 10^-6.5, negligible)."""
    for k in range(7):
        Ls = [0.0] * 7
        Ls[k] = s.ART_DB[k]
        c = s.correct_mtf(np.ones((7, NF)), Ls)
        assert c["factor"][k] == pytest.approx(0.5, rel=1e-6)


def test_speech_spectrum_table_A4():
    """Table A.4 at 60 dB(A) (J.3 default) and at 70 dB(A) (J.3 raised effort). Female 125 Hz is '-'."""
    np.testing.assert_allclose(s.speech_spectrum("male"), [62.9, 62.9, 59.2, 53.2, 47.2, 41.2, 35.2])
    f = s.speech_spectrum("female")
    assert math.isnan(f[0])
    np.testing.assert_allclose(f[1:], [65.3, 58.1, 50.9, 44.2, 43.3, 42.0])
    np.testing.assert_allclose(s.speech_spectrum("male", 70.0), [72.9, 72.9, 69.2, 63.2, 57.2, 51.2, 45.2])


def test_speech_spectrum_is_zero_dBA():
    """A.3.5: 'normalized to an A-weighted level of 0 dB'. Octave A-weights (IEC 61672, not in hand; the usual
    rounded octave values -16.1 -8.6 -3.2 0 +1.2 +1.0 -1.1) give male +0.05 dB and female -0.11 dB."""
    aw = np.array([-16.1, -8.6, -3.2, 0.0, 1.2, 1.0, -1.1])
    for sex, want in (("male", 0.05), ("female", -0.11)):
        L = s.speech_spectrum(sex, 0.0) + aw
        tot = 10 * np.log10(np.nansum(10 ** (L / 10)))
        assert tot == pytest.approx(want, abs=0.01)


# --- 6.1 / A.1.2 MTF from the IR -------------------------------------------------------------------------

@pytest.mark.parametrize("T", [0.3, 1.0, 2.5])
def test_exponential_decay_A12(T):
    """A.1.2 (Figure A.2 case A): energy decay E(t) ~ exp(-13.8 t / T) gives m(F) = 1/sqrt(1+(2 pi F T/13.8)^2).
    Discrete sum (6.1, energy form): with a = exp(-13.8 dt/T), m = (1-a)/|1 - a e^{-j 2 pi F dt}| exactly
    (geometric series, tail a^N negligible at N dt = 8T, i.e. 480 dB down). The continuous limit is reached as
    dt -> 0; at dt = 10 us the bin error is < 1e-4."""
    dt = 1e-5
    N = int(round(8 * T / dt))
    a = math.exp(-13.8 * dt / T)
    E = a ** np.arange(N)
    m = s.mtf_from_energy_ir(E, dt)
    F = np.asarray(s.MOD_FREQS_HZ)
    exact_discrete = (1 - a) / np.abs(1 - a * np.exp(-2j * np.pi * F * dt))
    np.testing.assert_allclose(m, exact_discrete, rtol=1e-9)
    np.testing.assert_allclose(m, s.exponential_decay_mtf(F, T), atol=1e-4)
    # spot value, hand-computed: T = 1 s, F = 1 Hz -> 2 pi/13.8 = 0.45530, 1/sqrt(1.20730) = 0.91011
    if T == 1.0:
        assert m[2] == pytest.approx(1 / math.sqrt(1 + (2 * math.pi / 13.8) ** 2), abs=1e-4)
        assert m[2] == pytest.approx(0.91011, abs=1e-4)


def test_time_shift_invariance_6_1():
    """6.1: |integral| is invariant to where t=0 sits, so leading empty bins do not change m."""
    rng = np.random.default_rng(1)
    E = rng.random(5000) * np.exp(-np.arange(5000) / 800.0)
    m0 = s.mtf_from_energy_ir(E, 1e-3)
    m1 = s.mtf_from_energy_ir(np.concatenate([np.zeros(137), E]), 1e-3)
    np.testing.assert_allclose(m0, m1, rtol=1e-12)


def test_delta_ir_m_is_one():
    """6.1: a single-bin IR has |sum| = sum, m = 1 at every F."""
    np.testing.assert_allclose(s.mtf_from_energy_ir([3.7], 1e-3), np.ones(NF))


def test_two_pulse_echo():
    """A.1.2 (echo -> notch): two equal pulses dt_e apart give m(F) = |cos(pi F dt_e)|. dt_e = 0.2 s:
    F = 2.5 Hz -> cos(pi/2) = 0; F = 1 Hz -> cos(0.2 pi) = 0.80902."""
    dt = 1e-3
    E = np.zeros(1000)
    E[0] = E[200] = 1.0
    m = s.mtf_from_energy_ir(E, dt)
    assert m[6] == pytest.approx(0.0, abs=1e-12)  # 2.5 Hz
    assert m[2] == pytest.approx(0.80902, abs=1e-5)  # 1.0 Hz


def test_noise_factor_A12_case_B():
    """6.1 second factor / A.1.2 case B: noise only, m = 1/(1+10^(-SNR/10)). SNR 0 dB -> 0.5;
    SNR 10 dB -> 1/1.1 = 0.90909. Auditory effects off so only noise acts (A.3.1 NOTE)."""
    Ls = [60.0] * 7
    Ln = [60.0, 50.0, 60.0, 50.0, 60.0, 50.0, 60.0]
    c = s.correct_mtf(np.ones((7, NF)), Ls, Ln, auditory_effects=False)
    np.testing.assert_allclose(c["factor"], [0.5, 1 / 1.1, 0.5, 1 / 1.1, 0.5, 1 / 1.1, 0.5], rtol=1e-12)
    np.testing.assert_allclose(c["m_noise"], s.noise_mtf(np.array(Ls) - np.array(Ln)), rtol=1e-12)


# --- A.5.4 - A.5.6: SNR clip, TI, MTI, STI ---------------------------------------------------------------

def test_m_one_gives_sti_one_both_sexes():
    """A.5.4-A.5.6 + Table A.3: m' = 1 -> SNR clipped to +15 -> TI = 1 -> MTI = 1 ->
    STI = sum(alpha) - sum(beta) = 1.000 for male and female. Auditory effects disabled (A.3.1 NOTE), no noise."""
    r = s.sti_reference(delta_irs(), 1e-3, None, None, auditory_effects=False)
    np.testing.assert_allclose(r["m_ir"], 1.0)
    np.testing.assert_allclose(r["snr_eff"], 15.0)
    np.testing.assert_allclose(r["ti"], 1.0)
    np.testing.assert_allclose(r["mti"], 1.0)
    assert r["male"]["sti_raw"] == pytest.approx(1.000, abs=1e-12)
    assert r["female"]["sti_raw"] == pytest.approx(1.000, abs=1e-12)
    assert r["sti_male"] == pytest.approx(1.0, abs=1e-12)
    assert r["sti_female"] == pytest.approx(1.0, abs=1e-12)


def test_m_zero_gives_sti_zero():
    """A.5.4: m' = 0 -> SNR = -inf, limited to -15 dB -> TI = 0 (A.5.5) -> MTI = 0 -> STI = 0 (A.5.6)."""
    r = s.sti_from_mtf(np.zeros((7, NF)), None, None, auditory_effects=False)
    np.testing.assert_allclose(r["snr_eff"], -15.0)
    np.testing.assert_allclose(r["ti"], 0.0)
    assert r["sti_male"] == 0.0
    assert r["sti_female"] == 0.0


def test_snr_clip_A54():
    """A.5.4: SNR_eff = 10 lg(m'/(1-m')) limited to [-15, +15] dB.
    m' = 0.5 -> 0 dB; 0.99 -> 19.956 -> 15; 0.01 -> -19.956 -> -15; m' = 10^1.5/(1+10^1.5) -> exactly 15;
    m' = 0.9 -> 9.5424 (not clipped)."""
    m = np.array([[0.5, 0.99, 0.01, 10 ** 1.5 / (1 + 10 ** 1.5), 0.9]])
    raw, clipped = s.snr_eff(m)
    np.testing.assert_allclose(raw[0, [0, 1, 2, 4]], [0.0, 19.956, -19.956, 9.5424], atol=1e-3)
    np.testing.assert_allclose(clipped[0], [0.0, 15.0, -15.0, 15.0, 9.5424], atol=1e-3)
    np.testing.assert_allclose(s.transmission_index(clipped[0]), [0.5, 1.0, 0.0, 1.0, 24.5424 / 30], atol=1e-4)


def test_m_above_one_truncated_A53_note1():
    """A.5.3 NOTE 1: m-values > 1.0 truncated to 1.0 -> TI = 1."""
    r = s.sti_from_mtf(np.full((7, NF), 1.2), None, None, auditory_effects=False)
    np.testing.assert_allclose(r["m_corrected"], 1.0)
    assert r["sti_male"] == pytest.approx(1.0)


def test_mti_is_mean_over_F_A56():
    """A.5.6: MTI_k = (1/14) sum TI. TI 0..13/13 linearly -> mean 0.5."""
    ti = np.tile(np.linspace(0, 1, NF), (7, 1))
    np.testing.assert_allclose(s.modulation_transfer_index(ti), 0.5)


def test_sti_formula_hand_example_A56():
    """A.5.6: STI = sum a_k MTI_k - sum b_k sqrt(MTI_k MTI_k+1). MTI all 0.5 -> (1.381 - 0.381) * 0.5 = 0.5.
    MTI = [1, .25, 1, .25, 1, .25, 1] male: sum a MTI = .085+.03175+.23+.05825+.309+.056+.173 = 0.943;
    every adjacent product is .25 -> sqrt .5 -> sum b * .5 = 0.1905; STI = 0.7525."""
    assert s.sti_from_mti([0.5] * 7, "male")["sti"] == pytest.approx(0.5, abs=1e-12)
    assert s.sti_from_mti([0.5] * 7, "female")["sti"] == pytest.approx(0.5, abs=1e-12)
    r = s.sti_from_mti([1, .25, 1, .25, 1, .25, 1], "male")
    assert r["sum_alpha_mti"] == pytest.approx(0.943, abs=1e-12)
    assert r["sum_beta_mti"] == pytest.approx(0.1905, abs=1e-12)
    assert r["sti"] == pytest.approx(0.7525, abs=1e-12)


def test_female_ignores_125():
    """Table A.3: female has no 125 Hz alpha/beta; its MTI (even NaN) must not change female STI."""
    a = s.sti_from_mti([0.0, .6, .6, .6, .6, .6, .6], "female")["sti"]
    b = s.sti_from_mti([float("nan"), .6, .6, .6, .6, .6, .6], "female")["sti"]
    assert a == pytest.approx(0.6, abs=1e-12) and b == pytest.approx(0.6, abs=1e-12)


def test_male_artefact_note_table_A3():
    """Table A.3 NOTE: 250 Hz removed (MTI_250 = 0), all others 1 -> male STI = 1.381-0.127 - (0.065+0.011
    +0.047+0.095) = 1.036 ('1,03' in the note), truncated to 1.0 (also A.5.6 NOTE). The crossing to STI > 1 is
    at MTI_250 = 0.0804 (root of 0.127x - 0.163 sqrt(x) + 0.036 = 0)."""
    r = s.sti_from_mti([1, 0, 1, 1, 1, 1, 1], "male")
    assert r["sti_raw"] == pytest.approx(1.036, abs=1e-12)
    assert r["sti"] == 1.0
    assert s.sti_from_mti([1, 0.0804, 1, 1, 1, 1, 1], "male")["sti_raw"] == pytest.approx(1.0, abs=1e-4)
    assert s.sti_from_mti([1, 0.147, 1, 1, 1, 1, 1], "male")["sti_raw"] < 1.0


# --- A.3.2 / Table A.1 masking ---------------------------------------------------------------------------

@pytest.mark.parametrize("L, want", [
    (50.0, -40.0),     # <63: 0.5*50 - 65
    (62.9, -33.55),    # <63
    (63.0, -33.5),     # 63<=L<67: 1.8*63 - 146.9 = -33.5 (meets branch 1 at 63)
    (65.0, -29.9),     # 1.8*65 - 146.9
    (67.0, -26.3),     # 67<=L<100: 0.5*67 - 59.8 = -26.3 (meets branch 2 at 67)
    (80.0, -19.8),
    (99.9, -9.85),
    (100.0, -10.0),    # >=100: -10 (0.2 dB step from -9.8, as printed)
    (120.0, -10.0),
])
def test_masking_table_A1_branches(L, want):
    """Table A.1, every branch and boundary, hand-computed."""
    assert s.masking_db(L) == pytest.approx(want, abs=1e-9)
    assert s.masking_factor(L) == pytest.approx(10 ** (want / 10), rel=1e-9)


def test_masking_uses_band_below_A32():
    """A.3.2: I_am,k = I_(k-1) * amf(L_(k-1)); 125 Hz is not masked; band k's own level does not set its masking.
    L_s = [80, 40, 90, 40, ...]: amdB(80) = -19.8 -> I_am,250 = 10^8 * 10^-1.98 = 10^6.02.
    m'_250 = 10^4 / (10^4 + 10^6.02 + 10^2.7) = 0.0094."""
    Ls = [80.0, 40.0, 90.0, 40.0, 40.0, 40.0, 40.0]
    c = s.correct_mtf(np.ones((7, NF)), Ls)
    assert c["I_am"][0] == 0.0
    assert c["I_am"][1] == pytest.approx(10 ** 6.02, rel=1e-12)
    assert c["factor"][1] == pytest.approx(1e4 / (1e4 + 10 ** 6.02 + 10 ** 2.7), rel=1e-12)
    assert c["factor"][1] == pytest.approx(0.0094, abs=1e-4)
    # 500 Hz masked by the 40 dB 250 band: amdB(40) = -45 -> I_am = 10^4 * 10^-4.5 = 10^-0.5
    assert c["I_am"][2] == pytest.approx(10 ** -0.5, rel=1e-12)
    # 1 kHz masked by the 90 dB 500 band: amdB(90) = -14.8 -> I_am = 10^9 * 10^-1.48 = 10^7.52
    assert c["I_am"][3] == pytest.approx(10 ** 7.52, rel=1e-12)
    # raising band 250's own level leaves I_am,250 unchanged
    Ls2 = list(Ls)
    Ls2[1] = 70.0
    assert s.correct_mtf(np.ones((7, NF)), Ls2)["I_am"][1] == pytest.approx(c["I_am"][1], rel=1e-15)


def test_masking_level_includes_noise_A32():
    """A.3.2 last paragraph and NOTE 1: the masking level is band k-1's signal + noise.
    L_s,125 = L_n,125 = 77 dB -> total 80.0103 dB; amdB = 0.5*80.0103 - 59.8 = -19.7949."""
    Ls = [77.0] + [60.0] * 6
    Ln = [77.0] + [0.0] * 6
    c = s.correct_mtf(np.ones((7, NF)), Ls, Ln)
    Ltot = 77.0 + 10 * math.log10(2)
    assert c["L_total"][0] == pytest.approx(Ltot, abs=1e-12)
    assert c["amdB"][1] == pytest.approx(0.5 * Ltot - 59.8, abs=1e-12)
    assert c["I_am"][1] == pytest.approx(2 * 10 ** 7.7 * 10 ** ((0.5 * Ltot - 59.8) / 10), rel=1e-12)


def test_A53_note2_noise_in_denominator():
    """A.5.3 NOTE 2: m' = m I_s/(I_s + I_am + I_rt + I_n). Annex M's split noise x (masking+threshold with
    I_k = I_s + I_n) is the same product."""
    Ls = [70.0, 65.0, 60.0, 55.0, 50.0, 45.0, 40.0]
    Ln = [50.0, 45.0, 40.0, 35.0, 30.0, 25.0, 20.0]
    c = s.correct_mtf(np.ones((7, NF)), Ls, Ln)
    np.testing.assert_allclose(c["factor"], c["m_noise"] * c["m_masking_threshold"], rtol=1e-12)
    Is, In = 10 ** (np.array(Ls) / 10), 10 ** (np.array(Ln) / 10)
    np.testing.assert_allclose(c["factor"], Is / (Is + In + c["I_am"] + c["I_rt"]), rtol=1e-12)


# --- 6.2 b / 8.3 a duration ------------------------------------------------------------------------------

def test_duration_flags_8_3a():
    """8.3 a / 6.2 b: IR at least 1.6 s and not less than T/2."""
    short = [np.ones(1599) for _ in s.BANDS_HZ]
    ok = [np.ones(1600) for _ in s.BANDS_HZ]
    assert s.sti_reference(short, 1e-3, None, auditory_effects=False)["duration_ok_1_6s"] is False
    r = s.sti_reference(ok, 1e-3, None, auditory_effects=False, reverberation_time_s=4.0)
    assert r["duration_ok_1_6s"] is True and r["duration_ok_half_T"] is False


# --- Annex M, Table M.1 worked example -------------------------------------------------------------------

BANDS = 7


def assert_sig(actual, printed, sig):
    """Each actual value rounds to the printed value at `sig` significant figures (half a unit of the last
    printed digit)."""
    actual = np.asarray(actual, dtype=float)
    printed = np.asarray(printed, dtype=float)
    sig = np.broadcast_to(np.asarray(sig), printed.shape)
    half_ulp = 0.5 * 10.0 ** (np.floor(np.log10(np.abs(printed))) - sig + 1)
    assert np.all(np.abs(actual - printed) <= half_ulp + 1e-12 * np.abs(printed)), (actual, printed)
# Step 1: measurement levels and MTF (rows F 0.63..12.5, columns 125..8000), transposed to (band, F)
M_L_S1 = [77.9, 77.9, 74.2, 68.2, 62.2, 56.2, 50.2]
M_L_N1 = [48.0, 40.0, 34.0, 30.0, 27.0, 25.0, 23.0]
M_MTF1 = np.array([
    [0.982, 0.952, 0.960, 0.969, 0.979, 0.983, 0.994],
    [0.966, 0.928, 0.941, 0.954, 0.969, 0.976, 0.992],
    [0.945, 0.897, 0.914, 0.933, 0.955, 0.965, 0.989],
    [0.919, 0.862, 0.881, 0.908, 0.939, 0.952, 0.984],
    [0.884, 0.819, 0.836, 0.873, 0.915, 0.932, 0.978],
    [0.850, 0.784, 0.793, 0.838, 0.890, 0.911, 0.971],
    [0.815, 0.750, 0.749, 0.799, 0.862, 0.888, 0.961],
    [0.772, 0.715, 0.716, 0.760, 0.832, 0.863, 0.950],
    [0.740, 0.678, 0.691, 0.730, 0.800, 0.836, 0.938],
    [0.724, 0.623, 0.665, 0.721, 0.772, 0.811, 0.926],
    [0.713, 0.553, 0.643, 0.708, 0.745, 0.785, 0.913],
    [0.669, 0.515, 0.611, 0.664, 0.720, 0.764, 0.901],
    [0.590, 0.479, 0.545, 0.603, 0.693, 0.748, 0.890],
    [0.553, 0.442, 0.513, 0.602, 0.678, 0.736, 0.881]]).T
# Step 2: adjusted MTF without noise, masking and threshold
M_MTF2 = np.array([
    [0.983, 0.960, 0.978, 0.990, 0.990, 0.986, 0.997],
    [0.968, 0.936, 0.959, 0.974, 0.980, 0.979, 0.995],
    [0.947, 0.904, 0.931, 0.953, 0.966, 0.968, 0.992],
    [0.920, 0.869, 0.898, 0.927, 0.949, 0.955, 0.987],
    [0.886, 0.826, 0.852, 0.892, 0.925, 0.935, 0.981],
    [0.851, 0.791, 0.808, 0.856, 0.900, 0.914, 0.974],
    [0.816, 0.756, 0.764, 0.816, 0.871, 0.891, 0.964],
    [0.773, 0.721, 0.730, 0.776, 0.841, 0.866, 0.953],
    [0.741, 0.684, 0.705, 0.745, 0.809, 0.838, 0.941],
    [0.726, 0.628, 0.678, 0.736, 0.780, 0.812, 0.929],
    [0.714, 0.557, 0.656, 0.723, 0.753, 0.786, 0.916],
    [0.670, 0.520, 0.623, 0.678, 0.728, 0.765, 0.904],
    [0.591, 0.483, 0.556, 0.615, 0.701, 0.749, 0.893],
    [0.554, 0.446, 0.523, 0.614, 0.685, 0.737, 0.884]]).T
# Step 3: operational levels and the resulting MTF
M_L_S3 = [82.9, 82.9, 79.2, 73.2, 67.2, 61.2, 55.2]
M_L_N3 = [55.5, 47.5, 41.5, 37.5, 34.5, 32.5, 30.5]
M_MTF3 = np.array([
    [0.981, 0.946, 0.946, 0.953, 0.971, 0.975, 0.992],
    [0.966, 0.922, 0.927, 0.938, 0.961, 0.968, 0.990],
    [0.945, 0.891, 0.900, 0.918, 0.947, 0.957, 0.987],
    [0.919, 0.856, 0.868, 0.893, 0.931, 0.944, 0.982],
    [0.884, 0.814, 0.823, 0.859, 0.907, 0.925, 0.976],
    [0.850, 0.779, 0.781, 0.824, 0.882, 0.904, 0.969],
    [0.814, 0.745, 0.738, 0.786, 0.855, 0.881, 0.959],
    [0.772, 0.710, 0.706, 0.747, 0.825, 0.856, 0.948],
    [0.739, 0.674, 0.681, 0.718, 0.793, 0.829, 0.936],
    [0.724, 0.619, 0.656, 0.709, 0.765, 0.804, 0.924],
    [0.713, 0.549, 0.634, 0.696, 0.739, 0.778, 0.911],
    [0.668, 0.512, 0.602, 0.653, 0.714, 0.757, 0.900],
    [0.589, 0.476, 0.537, 0.593, 0.687, 0.741, 0.889],
    [0.553, 0.439, 0.505, 0.592, 0.672, 0.729, 0.880]]).T
# Step 4a: effective SNR printed to 2 dp
M_SNR4A = np.array([
    [17.21, 12.44, 12.42, 13.09, 15.21, 15.93, 21.01],
    [14.55, 10.73, 11.04, 11.83, 13.90, 14.83, 20.02],
    [12.34, 9.13, 9.56, 10.47, 12.52, 13.50, 18.86],
    [10.52, 7.74, 8.17, 9.22, 11.31, 12.30, 17.41],
    [8.82, 6.41, 6.69, 7.84, 9.91, 10.88, 16.13],
    [7.52, 5.47, 5.52, 6.71, 8.76, 9.73, 14.98],
    [6.42, 4.66, 4.51, 5.64, 7.70, 8.69, 13.72],
    [5.29, 3.89, 3.80, 4.71, 6.73, 7.75, 12.64],
    [4.53, 3.16, 3.30, 4.06, 5.84, 6.87, 11.68],
    [4.19, 2.11, 2.79, 3.87, 5.14, 6.12, 10.87],
    [3.95, 0.85, 2.38, 3.60, 4.51, 5.44, 10.13],
    [3.04, 0.21, 1.80, 2.74, 3.97, 4.94, 9.52],
    [1.57, -0.42, 0.65, 1.63, 3.42, 4.57, 9.02],
    [0.92, -1.06, 0.10, 1.61, 3.12, 4.31, 8.64]]).T
M_MTI = [0.73, 0.66, 0.67, 0.71, 0.77, 0.80, 0.92]


def test_annexM_step2_masking_rows():
    """Annex M step 2 (measurement levels): combined level, amdB, I_am, the masking+threshold adjustment,
    the noise-only m and the combined adjustment, each to its printed precision."""
    c = s.correct_mtf(np.ones((7, NF)), M_L_S1, M_L_N1)
    np.testing.assert_allclose(c["L_total"], [77.90, 77.90, 74.20, 68.20, 62.20, 56.20, 50.21], atol=0.005)
    np.testing.assert_allclose(c["amdB"][1:], [-20.8, -20.8, -22.7, -25.7, -33.9, -36.9], atol=0.05)
    assert_sig(c["amf"][2:] * 1000, [8.22, 5.37, 2.69, 0.407, 0.204], 3)
    # 250 Hz: the print 8.22 is amf at the ROUNDED combined level 77.90 dB (10^-2.085 = 8.2224e-3); at the exact
    # noise-inclusive level 77.9044 dB (A.3.2) amf = 8.2266e-3, which rounds to 8.23. 0.002 dB of level.
    assert c["amf"][1] * 1000 == pytest.approx(8.2266, abs=1e-4)
    assert 10 ** ((0.5 * 77.90 - 59.8) / 10) * 1000 == pytest.approx(8.22, abs=0.005)
    assert_sig(c["I_am"][1:], [508000, 507000, 141000, 17800, 676, 85.2], 3)
    assert_sig(c["I_rt"], [40000, 501, 15.8, 4.5, 5.6, 6.3, 15.8], [2, 3, 3, 2, 2, 2, 3])
    np.testing.assert_allclose(1 / c["m_masking_threshold"], [1.001, 1.008, 1.019, 1.021, 1.011, 1.002, 1.001],
                               atol=0.0005)
    np.testing.assert_allclose(c["m_noise"], [0.999, 1.000, 1.000, 1.000, 1.000, 0.999, 0.998], atol=0.0005)
    np.testing.assert_allclose(1 / c["factor"], [1.002, 1.008, 1.019, 1.022, 1.011, 1.002, 1.003],
                               atol=0.0005 + 1e-6)


def test_annexM_step3_masking_rows():
    """Annex M step 3 (operational levels): amdB, amf, I_am, the masking+threshold correction, noise-only m,
    combined adjustment. I_am,250 prints 2 850 000 where the exact value is 2 858 800 (= I_125 incl. noise
    1.953e8 x 0.014635); 195 x 14.6 = 2 847 000 reproduces the print, so the table rounded its inputs first."""
    c = s.correct_mtf(np.ones((7, NF)), M_L_S3, M_L_N3)
    np.testing.assert_allclose(c["amdB"][1:], [-18.3, -18.3, -20.2, -23.2, -26.2, -34.4], atol=0.05)
    assert_sig(c["amf"][1:] * 1000, [14.6, 14.6, 9.55, 4.79, 2.40, 0.363], 3)
    assert_sig(c["I_am"][2:], [2850000, 795000, 100000, 12600, 480], [3, 3, 3, 3, 2])
    assert c["I_am"][1] == pytest.approx(2858800, rel=1e-4)  # printed 2 850 000: see docstring
    assert 195e6 * 14.6e-3 == pytest.approx(2847000)
    np.testing.assert_allclose(c["m_masking_threshold"], [1.000, 0.986, 0.967, 0.963, 0.981, 0.991, 0.999],
                               atol=0.0005)
    np.testing.assert_allclose(c["m_noise"], [0.998, 1.000, 1.000, 1.000, 0.999, 0.999, 0.997], atol=0.0005)
    np.testing.assert_allclose(c["factor"], [0.998, 0.985, 0.967, 0.963, 0.981, 0.989, 0.995], atol=0.0005)


def test_annexM_step2_to_step1_and_step3_matrices():
    """Annex M: the noise-free matrix (step 2) corrected at the measurement levels gives step 1's matrix, and at
    the operational levels gives step 3's. Inputs and outputs are both printed to 3 dp, so the bound is
    0.0005 (input) + 0.0005 (output) = 0.001."""
    c1 = s.correct_mtf(M_MTF2, M_L_S1, M_L_N1)
    np.testing.assert_allclose(c1["m_corrected"], M_MTF1, atol=0.001)
    c3 = s.correct_mtf(M_MTF2, M_L_S3, M_L_N3)
    np.testing.assert_allclose(c3["m_corrected"], M_MTF3, atol=0.001)


def test_annexM_step4_snr_mti_sti():
    """Annex M step 4 from the printed step-3 matrix: 4a SNR_eff (2 dp; the print was made from unrounded m, so
    each cell's bound is |dSNR/dm| x 0.0005 + 0.005 with dSNR/dm = 4.343/(m(1-m))); 4b cells printed > 15 are
    limited to 15; MTI_k = 0.73 0.66 0.67 0.71 0.77 0.80 0.92; sum alpha MTI = 1.040, sum beta = 0.282
    (A.5.6 sqrt form: 0.085 sqrt(.73 x .66) = 0.059 as printed), STI = 0.76."""
    raw, snr = s.snr_eff(M_MTF3)
    tol = 10 / math.log(10) / (M_MTF3 * (1 - M_MTF3)) * 0.0005 + 0.005
    assert np.all(np.abs(raw - M_SNR4A) <= tol)
    assert np.all(snr[M_SNR4A > 15.0] == 15.0)
    ti = s.transmission_index(snr)
    mti = s.modulation_transfer_index(ti)
    np.testing.assert_allclose(mti, M_MTI, atol=0.005)
    r = s.sti_from_mti(mti, "male")
    assert round(r["sum_alpha_mti"], 3) == 1.040
    assert round(r["sum_beta_mti"], 3) == 0.282
    assert round(r["sti"], 2) == 0.76
    # printed per-band products, from the unrounded MTI (the 2 dp MTI would give 0.0815 at 4 kHz)
    a = np.array(s.ALPHA["male"]) * mti
    np.testing.assert_allclose(a, [0.062, 0.083, 0.155, 0.165, 0.237, 0.179, 0.159], atol=0.0005)
    b = np.array(s.BETA["male"]) * np.sqrt(mti[:-1] * mti[1:])
    np.testing.assert_allclose(b, [0.059, 0.052, 0.045, 0.008, 0.037, 0.081], atol=0.0005)
    # the beta row is the sqrt form of A.5.6, not beta x MTI_k (0.085 x 0.73 = 0.062 would not print 0.059)
    assert 0.085 * 0.73 == pytest.approx(0.062, abs=0.0005)


def test_annexM_end_to_end():
    """Annex M steps 3-4 in one call: noise-free step-2 matrix + operational levels -> STI 0.76 (male)."""
    r = s.sti_from_mtf(M_MTF2, M_L_S3, M_L_N3)
    np.testing.assert_allclose(r["mti"], M_MTI, atol=0.005)
    assert round(r["sti_male"], 2) == 0.76


# --- full pipeline from an IR ----------------------------------------------------------------------------

def test_pipeline_exponential_with_levels():
    """Cl. 8.3 chain from IRs: exponential decays (T = 1 s), male speech at 60 dB(A) (Table A.4, J.3), no noise.
    Hand chain for the 1 kHz band: m(F) from A.1.2; masking from the 500 Hz band at 59.2 dB:
    amdB = 0.5*59.2 - 65 = -35.4; I_am = 10^5.92 * 10^-3.54 = 10^2.38; I_rt = 10^0.65; I_s = 10^5.32;
    factor = 10^5.32/(10^5.32 + 10^2.38 + 10^0.65) = 0.998832."""
    T, dt = 1.0, 1e-4
    E = np.exp(-13.8 * np.arange(int(2.0 / dt)) * dt / T)
    r = s.sti_reference([E] * 7, dt, s.speech_spectrum("male"))
    want_factor = 10 ** 5.32 / (10 ** 5.32 + 10 ** 2.38 + 10 ** 0.65)
    assert r["factor"][3] == pytest.approx(want_factor, rel=1e-12)
    assert want_factor == pytest.approx(0.998832, abs=1e-6)
    F = np.asarray(s.MOD_FREQS_HZ)
    np.testing.assert_allclose(r["m_ir"][3], s.exponential_decay_mtf(F, T), atol=2e-4)
    m1k = s.exponential_decay_mtf(F, T) * want_factor
    snr = np.clip(10 * np.log10(m1k / (1 - m1k)), -15, 15)
    assert r["mti"][3] == pytest.approx(np.mean((snr + 15) / 30), abs=2e-3)
    assert r["duration_ok_1_6s"] is True
    assert 0.0 < r["sti_male"] < 1.0 and 0.0 < r["sti_female"] < 1.0
