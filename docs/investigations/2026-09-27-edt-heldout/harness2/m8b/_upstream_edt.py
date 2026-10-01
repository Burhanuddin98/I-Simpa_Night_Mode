"""Exact float32 port of upstream I-Simpa's EDT (and TR) computation, for measurement only.

Upstream source (read-only checkout B:/repos/I-Simpa-upstream, tag v1.4.0_snapshot_14_01_2026):
  src/isimpa/data_manager/projet_calculation.cpp
    41      p_0 = 1/pow((float)(20*pow(10.f,-6)),2)           const float
    46-49   to_deciBelP0(w) = 10*log10f(w*p_0)                 float
    68-83   MakeSchroederArray: float sumW, backward, dB per step
    95-119  GetSumLimit: fromTime <= t <= toTime (inclusive), break once t > toTime
    143-170 GetTimeRange: reference row_db[0]; start = first step with |L-L0| >= fromdB
            (fromdB = 0 -> step 0, always); end = first LATER step with |L-L0| >= todB,
            else the last step
    175-186 ComputeLinearRegression: least squares in float32 (n int32)
    190-219 Compute_TR_Param: value = (-60/a)/1000 per band; last row = mean of the bands
    866-871 point receivers: timeTable parsed from the 'Sound level.recp' row labels
    936     EDT = Compute_TR_Param(0, 10, ...)  (point receivers)
    1020-1024, 1054  surface receivers: timeTable = timeStep*(i+1)*1000, EDT = (0, 10)
  src/lib_interface/input_output/baseReportManager.cpp:428  the labels the solver writes:
            FromFloat(pasdetemps*(float)(idstep+1)*1000.f, 1) + " ms"  ("%.1f", coreString.cpp:70-80)
  src/isimpa/manager/sppsString.cpp:43-57  Convertor::ToFloat (classic locale) -> float
So timeTable[i] is the END of step i in ms, rounded to 0.1 ms, stored as float32.

Nothing here runs a solver. Read-only on every input.
"""
import math
import struct

import numpy as np

F32 = np.float32
# projet_calculation.cpp:41 (C++11 <cmath>: pow(float,int) promotes to double; result stored as float)
P_0 = F32(1.0 / (float(F32(20 * 1e-6)) ** 2))
P_REF_SURF = F32(10.0) ** F32(-12.0)       # to_deciBelRsurf, line 52


def to_db_p0(w):
    """to_deciBelP0: 10*log10f(w*p_0), every step float32."""
    w = np.asarray(w, dtype=F32)
    with np.errstate(divide='ignore', invalid='ignore'):
        return (F32(10) * np.log10((w * P_0).astype(F32))).astype(F32)


def to_db_rsurf(w):
    w = np.asarray(w, dtype=F32)
    with np.errstate(divide='ignore', invalid='ignore'):
        return (F32(10) * np.log10((w / P_REF_SURF).astype(F32))).astype(F32)


def schroeder(row, conv=to_db_p0):
    """MakeSchroederArray (68-83) for one row: float sumW accumulated from the last step backwards.
    np.add.accumulate on float32 is a sequential float32 sum, the same order as the C++ loop."""
    w = np.asarray(row, dtype=F32)
    sums = np.add.accumulate(w[::-1], dtype=F32)[::-1]
    return conv(sums), sums


_TT = {}


def point_time_table(dt, n):
    """The labels SPPS writes for a point receiver (baseReportManager.cpp:428), parsed back as the
    GUI does (projet_calculation.cpp:866-871): float(pasdetemps*(float)(i+1)*1000.f) -> '%.1f' -> float.
    Entry i depends only on (dt, i), so tables are cached per dt and sliced."""
    dt32 = F32(dt)
    key = float(dt32)
    have = _TT.get(key)
    if have is None or len(have) < n:
        m = max(n, 2 * (len(have) if have is not None else 0), 4096)
        out = np.empty(m, dtype=F32)
        for i in range(m):
            v = F32(F32(dt32 * F32(i + 1)) * F32(1000.0))
            out[i] = F32(float('%.1f' % float(v)))
        _TT[key] = have = out
    return have[:n].copy()


def surface_time_table(dt, n):
    """projet_calculation.cpp:1020-1024: timeStep*(idstep+1)*1000 in float."""
    dt32 = F32(dt)
    return np.array([F32(F32(dt32 * F32(i + 1)) * F32(1000.0)) for i in range(n)], dtype=F32)


def get_time_range(from_db, to_db, tt, row_db):
    """GetTimeRange (143-170). Returns (timeDeb, timeEnd, i_start, i_end_or_None).
    abs() of a float difference; with integer thresholds an int-truncating abs() would give the
    same comparisons (|trunc(x)| >= n  <=>  |x| >= n for integer n)."""
    if len(row_db) == 0:
        return None
    from_db = F32(from_db); to_db = F32(to_db)
    deb = tt[0]; end = tt[len(tt) - 1]
    last = row_db[0]
    inside = False
    i_s, i_e = 0, None
    with np.errstate(invalid='ignore'):
        for i in range(len(row_db)):
            d = abs(F32(row_db[i] - last))
            if not inside:
                if d >= from_db:        # NaN (zero energy: -inf - -inf) compares False
                    deb = tt[i]; i_s = i
                    inside = True
            else:
                if d >= to_db:
                    end = tt[i]; i_e = i
                    break
    return deb, end, i_s, i_e


def get_sum_limit(from_t, to_t, tt, row, op):
    """GetSumLimit (95-119), float32 accumulation in step order; returns (sum, n)."""
    s = F32(0)
    n = 0
    with np.errstate(invalid='ignore', over='ignore'):
        for i in range(len(tt)):
            t = tt[i]
            if t >= from_t and (t <= to_t or to_t == F32(-1)):
                n += 1
                if op == 'Y':
                    s = F32(s + row[i])
                elif op == 'XY':
                    s = F32(s + F32(row[i] * t))
                elif op == 'X':
                    s = F32(s + t)
                elif op == 'X2':
                    s = F32(s + F32(t * t))
            if t > to_t and to_t != F32(-1):
                break
    return s, n


def regression(from_t, to_t, tt, row_db):
    """ComputeLinearRegression (175-186). Returns (a, b, n)."""
    Sx, n = get_sum_limit(from_t, to_t, tt, row_db, 'X')
    Sy, _ = get_sum_limit(from_t, to_t, tt, row_db, 'Y')
    Sx2, _ = get_sum_limit(from_t, to_t, tt, row_db, 'X2')
    Sxy, _ = get_sum_limit(from_t, to_t, tt, row_db, 'XY')
    nf = F32(n)
    with np.errstate(invalid='ignore', divide='ignore', over='ignore'):
        den = F32(F32(nf * Sx2) - F32(Sx * Sx))
        a = F32(F32(F32(nf * Sxy) - F32(Sx * Sy)) / den)
        b = F32(F32(F32(Sy * Sx2) - F32(Sx * Sxy)) / den)
    return a, b, n


def tr_param(from_db, to_db, tt, row_db):
    """One band of Compute_TR_Param (190-219): (value_s, detail)."""
    deb, end, i_s, i_e = get_time_range(from_db, to_db, tt, row_db)
    a, b, n = regression(deb, end, tt, row_db)
    with np.errstate(invalid='ignore', divide='ignore', over='ignore'):
        val = F32(F32(F32(-60) / a) / F32(1000))
    return val, dict(i_start=i_s, i_end=i_e, n_fit=n, slope_db_per_ms=float(a), t_start_ms=float(deb),
                     t_end_ms=float(end), reached=i_e is not None)


def edt(series, dt, kind='point', tt=None):
    """Upstream's EDT (s) of one energy series (one band), as the GUI computes it for a point
    receiver ('point', labels rounded to 0.1 ms) or a surface receiver ('surface')."""
    v = np.asarray(series, dtype=F32)
    if tt is None:
        tt = point_time_table(dt, len(v)) if kind == 'point' else surface_time_table(dt, len(v))
    conv = to_db_p0 if kind == 'point' else to_db_rsurf
    row_db, _ = schroeder(v, conv)
    val, det = tr_param(0, 10, tt, row_db)
    det['L0_db'] = float(row_db[0])
    if det['i_end'] is not None:
        det['L_end_rel_db'] = float(row_db[det['i_end']] - row_db[0])
    nz = np.nonzero(v > 0)[0]
    det['first_energy_step'] = int(nz[0]) if len(nz) else None
    det['n_steps'] = len(v)
    return float(val), det


def edt_f64(series, dt, kind='point'):
    """The same algorithm in float64 (for measuring the float32 rounding share only)."""
    v = np.asarray(series, dtype=np.float64)
    n = len(v)
    tt = point_time_table(dt, n).astype(np.float64) if kind == 'point' else surface_time_table(dt, n).astype(np.float64)
    s = np.cumsum(v[::-1])[::-1]
    with np.errstate(divide='ignore', invalid='ignore'):
        L = 10 * np.log10(s * float(P_0))
        d = np.abs(L - L[0])
    hit = np.nonzero(d[1:] >= 10)[0]
    ie = int(hit[0]) + 1 if len(hit) else n - 1
    x = tt[:ie + 1]; y = L[:ie + 1]
    m = len(x)
    with np.errstate(divide='ignore', invalid='ignore'):
        a = (m * (x * y).sum() - x.sum() * y.sum()) / (m * (x * x).sum() - x.sum() ** 2)
        return float(-60.0 / a / 1000.0)


def compute_tr_param_table(from_db, to_db, tt, tab_wj, conv=to_db_p0):
    """Compute_TR_Param over every band + the Global row (sum of bands, lines 876-900) + the
    Average (mean of band rows, Global excluded, lines 206-209). Returns list of float32."""
    rows = [np.asarray(r, dtype=F32) for r in tab_wj]
    glob = np.zeros(len(rows[0]), dtype=F32)
    for r in rows:                      # tab_wj[nbBandeFreq][idstep] += ..., band by band
        glob = (glob + r).astype(F32)
    rows = rows + [glob]
    vals = []
    s = F32(0)
    for i, r in enumerate(rows):
        db, _ = schroeder(r, conv)
        v, _ = tr_param(from_db, to_db, tt, db)
        if i != len(rows) - 1:
            s = F32(s + v)
        vals.append(v)
    vals.append(F32(s / F32(len(rows) - 1)))
    return vals, rows


# ------------------------------------------------------------------------------------------------
# GABE (lib_interface/input_output/gabe/gabe.cpp:353-440 Load; 126-141, 177-188, 200-211 columns)
# ------------------------------------------------------------------------------------------------
def read_gabe(path):
    b = open(path, 'rb').read()
    ver, fh_len, ch_len, cols = struct.unpack_from('<iiii', b, 0)
    off = 20                                     # + bool readOnly + 3 skipped bytes
    out = []
    for _ in range(cols):
        ct, = struct.unpack_from('<H', b, off)
        rows, = struct.unpack_from('<i', b, off + 4)
        sz, = struct.unpack_from('<q', b, off + 8)
        label = b[off + 24:off + 24 + 255].split(b'\0')[0].decode('latin-1')
        off += 24 + 255 + 1
        if ct == 50:                             # float: int32 numOfDigits + rows float32
            nd, = struct.unpack_from('<i', b, off)
            off += 4
            vals = np.frombuffer(b, dtype='<f4', count=rows, offset=off).copy()
            off += 4 * rows
            out.append(dict(type='float', label=label, digits=nd, data=vals))
        elif ct == 51:
            off += 1
            vals = np.frombuffer(b, dtype='<i4', count=rows, offset=off).copy()
            off += 4 * rows
            out.append(dict(type='int', label=label, data=vals))
        elif ct == 52:
            off += 1
            vals = [b[off + 50 * i:off + 50 * i + 50].split(b'\0')[0].decode('latin-1') for i in range(rows)]
            off += 50 * rows
            out.append(dict(type='str', label=label, data=vals))
        else:
            off += sz
            out.append(dict(type='unknown', label=label, data=None))
    return dict(version=ver, cols=out)


def time_table_from_labels(labels):
    """projet_calculation.cpp:866-871: Left(find(' ')) then Convertor::ToFloat -> float."""
    return np.array([F32(float(s[:s.find(' ')] if ' ' in s else s)) for s in labels], dtype=F32)
