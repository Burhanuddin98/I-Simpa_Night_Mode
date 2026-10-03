"""Tests of the G and dB(A) truths (../ADDENDUM-6-GDBA.md): python -B -m pytest -p no:cacheprovider test_gdba.py"""
import sys

sys.dont_write_bytecode = True

import math  # noqa: E402

import pytest  # noqa: E402

import gdba  # noqa: E402

OCTAVES = (31.5, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000)


def exact_base10(nominal):
    """IEC 61260 / 61672-1: the exact midband frequency 1000 * 10^(n/10) nearest the nominal one."""
    n = round(10 * math.log10(nominal / 1000.0))
    return 1000.0 * 10 ** (n / 10)


def test_a_weights_octaves_against_the_table():
    # IEC 61672-1:2013 Table 3 at the octave centres, as printed
    want = {31.5: -39.4, 63: -26.2, 125: -16.1, 250: -8.6, 500: -3.2, 1000: 0.0, 2000: 1.2, 4000: 1.0,
            8000: -1.1, 16000: -6.6}
    for f in OCTAVES:
        assert gdba.a_weight(f) == want[f]


def test_a_weight_table_is_annex_e_rounded():
    # every typed entry equals the standard's own closed form at the exact frequency, rounded to 0.1 dB
    for f, a in gdba.A_WEIGHT_TABLE.items():
        assert abs(gdba.a_weight_annex_e(exact_base10(f)) - a) <= 0.05 + 1e-9, (f, a, gdba.a_weight_annex_e(
            exact_base10(f)))
    assert abs(gdba.a_weight_annex_e(1000.0)) < 1e-12


def test_g_is_zero_for_the_free_field_level_at_10m():
    W, rc = 1e-3, gdba.rho_c(20.0, 101325.0)
    # the free-field mean-square pressure at 10 m of a point source: I = W / (4 pi r^2), p^2 = I rho c
    p2 = W / (4 * math.pi * 100.0) * rc
    spl = 10 * math.log10(p2 / 4e-10)
    assert gdba.g_truth(spl, W, rc) == pytest.approx(0.0, abs=1e-12)
    # at 20 m in the free field: -20 lg 2
    spl20 = 10 * math.log10(W / (4 * math.pi * 400.0) * rc / 4e-10)
    assert gdba.g_truth(spl20, W, rc) == pytest.approx(-20 * math.log10(2), abs=1e-12)
    # a 1 dB change of source power moves SPL and the free-field level together
    W2 = W * 10 ** 0.1
    spl2 = 10 * math.log10(W2 / (4 * math.pi * 100.0) * rc / 4e-10)
    assert gdba.g_truth(spl2, W2, rc) == pytest.approx(0.0, abs=1e-12)


def test_rho_c_and_the_constant():
    rc = gdba.rho_c(20.0, 101325.0)
    assert rc == pytest.approx(413.3, abs=0.1)
    # Lw - L10 = 10 lg(4 pi 100 p0^2 / (W0 rho c)): 30.85 dB at rho c ~ 413, ISO's "31" at rho c = 400 (to 0.01)
    assert 10 * math.log10(1e-12 / 1e-12) - gdba.free_field_10m_db(1e-12, rc) == pytest.approx(30.85, abs=0.01)
    assert -gdba.free_field_10m_db(1e-12, 400.0) == pytest.approx(30.99, abs=0.01)


def test_dba_sums():
    # one band: SPL plus its weight
    assert gdba.dba_truth({125: 70.0})[0] == pytest.approx(70.0 - 16.1, abs=1e-12)
    # six flat bands 125 Hz-4 kHz at 60 dB
    want = 10 * math.log10(sum(10 ** ((60 + a) / 10) for a in (-16.1, -8.6, -3.2, 0.0, 1.2, 1.0)))
    level, share = gdba.dba_truth({f: 60.0 for f in (125, 250, 500, 1000, 2000, 4000)})
    assert level == pytest.approx(want, abs=1e-12)
    assert sum(share.values()) == pytest.approx(1.0)


def test_classify_is_the_beds():
    assert gdba.classify(5.0, 4.5, 5.5, 7.0)['wrong_silent'] is True
    assert gdba.classify(5.0, 2.0, 8.0, 7.0)['wrong_silent'] is False
    assert gdba.classify(5.0, 4.5, 5.5, 5.58)['covered'] is True
    assert gdba.classify(None, None, None, 5.0)['answered'] is False
