mod common;

use common::fixture;
use simpa_core::formats::FormatError;
use simpa_core::formats::csbin::{self, Csbin, RecordType, StructLengths};

const LIB_CUT: &str = "upstream/lib_interface/rs_cut.csbin";
const PY_CUT: &str = "upstream/python_bindings/rs_cut.csbin";
const SPPS: &str = "solver-outputs/tutorial1/spps_1000hz.csbin";

fn load(rel: &str) -> Csbin {
    csbin::read_file(&fixture(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn bytes(rel: &str) -> Vec<u8> {
    std::fs::read(fixture(rel)).unwrap()
}

/// Bytes the model accounts for when every record is stepped by its stored length.
fn consumed(m: &Csbin) -> usize {
    let l = m.lengths;
    let mut n = l.header as usize + m.nodes.len() * l.node as usize;
    for r in &m.receivers {
        n += l.receiver as usize;
        for f in &r.faces {
            n += l.face as usize + f.records.len() * l.value as usize;
        }
    }
    n
}

fn total_records(m: &Csbin) -> usize {
    m.receivers
        .iter()
        .flat_map(|r| &r.faces)
        .map(|f| f.records.len())
        .sum()
}

fn f(bits: u32) -> f32 {
    f32::from_bits(bits)
}

// Every fixture is consumed exactly: no trailing bytes, native struct lengths, invariants hold.
fn check_common(rel: &str, m: &Csbin) {
    assert_eq!(
        consumed(m),
        bytes(rel).len(),
        "{rel}: model does not account for every byte"
    );
    assert_eq!(m.version, 3);
    assert_eq!(m.lengths, StructLengths::NATIVE);
    for r in &m.receivers {
        for face in &r.faces {
            assert!(face.vertices.iter().all(|&v| (v as usize) < m.nodes.len()));
            assert!(
                face.records
                    .iter()
                    .all(|x| u32::from(x.time_step) < m.time_step_count)
            );
            // Upstream's writers emit a face's records in increasing time step order.
            assert!(
                face.records
                    .windows(2)
                    .all(|w| w[0].time_step < w[1].time_step),
                "{rel}"
            );
        }
    }
}

#[test]
fn lib_interface_rs_cut() {
    let m = load(LIB_CUT);
    check_common(LIB_CUT, &m);
    // Receipts: the task's golden values, and upstream's io_test.cpp:546-556
    // (nbTimeStep 5, RECEPTEURS_RECORD_TYPE_SPL_STANDART).
    assert_eq!(m.nodes.len(), 30);
    assert_eq!(m.receivers.len(), 1);
    assert_eq!(m.time_step_count, 5);
    assert_eq!(m.time_step.to_bits(), 0x3dcc_cccd); // 0.1 s
    assert_eq!(m.record_type, RecordType::SplStandard);
    let r = &m.receivers[0];
    assert_eq!(r.name, b"Plane Receiver");
    assert_eq!(r.name_lossy(), "Plane Receiver");
    assert_eq!(r.xml_index, 3389);
    assert_eq!(r.faces.len(), 40);
    assert!(r.faces.iter().all(|face| face.records.len() == 5));
    assert_eq!(total_records(&m), 200);
    // First and last node, first face and its first two values, read off the hex dump.
    assert_eq!(m.nodes[0], [0.0, 4.0, f(0x3fcc_cccd)]);
    assert_eq!(m.nodes[1], [0.0, 3.0, f(0x3fcc_cccd)]);
    assert_eq!(m.nodes[29], [5.0, 0.0, f(0x3fcc_cccd)]);
    let f0 = &r.faces[0];
    assert_eq!(f0.vertices, [1, 0, 5]);
    assert_eq!(f0.records[0].time_step, 0);
    assert_eq!(f0.records[0].energy.to_bits(), 0x338e_9944);
    assert_eq!(f0.records[1].time_step, 1);
    assert_eq!(f0.records[1].energy.to_bits(), 0x30e3_22b9);
    assert_eq!(r.faces[39].vertices, [24, 28, 29]);
    assert_eq!(r.faces[39].records[4].energy.to_bits(), 0x2948_e575);
}

#[test]
fn python_bindings_rs_cut() {
    let m = load(PY_CUT);
    check_common(PY_CUT, &m);
    // Receipts: upstream's check_retrocompat_recsurf.py:16-24 (1 receiver, 1248 faces,
    // name "Récepteur coupe" in cp1252, xml id 920).
    assert_eq!(m.nodes.len(), 680);
    assert_eq!(m.receivers.len(), 1);
    assert_eq!(m.time_step_count, 50);
    assert_eq!(m.time_step.to_bits(), 0x3c23_d70a); // 0.01 s
    assert_eq!(m.record_type, RecordType::SplStandard);
    let r = &m.receivers[0];
    assert_eq!(r.faces.len(), 1248);
    assert_eq!(r.name, b"R\xe9cepteur coupe");
    assert_eq!(r.xml_index, 920);
    assert_eq!(total_records(&m), 60870);
    assert!(r.faces.iter().all(|face| !face.records.is_empty()));
    assert_eq!(m.nodes[0], [0.0, 8.0, f(0x3fcc_cccd)]);
    assert_eq!(
        m.nodes[679].map(f32::to_bits),
        [0x4198_c7ae, 0, 0x3fcc_cccd]
    );
    assert_eq!(r.faces[0].vertices, [1, 0, 17]);
    assert_eq!(r.faces[0].records[0].energy.to_bits(), 0x348b_2db8);
    assert_eq!(r.faces[1247].vertices, [662, 678, 679]);
    let last = r.faces[1247].records.last().unwrap();
    assert_eq!((last.time_step, last.energy.to_bits()), (49, 0x280a_b7ad));
}

#[test]
fn spps_tutorial1_1000hz() {
    let m = load(SPPS);
    check_common(SPPS, &m);
    assert_eq!(m.nodes.len(), 732);
    assert_eq!(m.receivers.len(), 1);
    assert_eq!(m.time_step_count, 200);
    assert_eq!(m.time_step.to_bits(), 0x3c23_d70a); // 0.01 s
    assert_eq!(m.record_type, RecordType::SplStandard);
    let r = &m.receivers[0];
    assert_eq!(r.name, b"Receiver");
    assert_eq!(r.xml_index, 3503);
    assert_eq!(r.faces.len(), 934);
    assert_eq!(total_records(&m), 6041);
    // One face recorded nothing: nbRecords 0 is valid.
    assert_eq!(
        r.faces
            .iter()
            .filter(|face| face.records.is_empty())
            .count(),
        1
    );
    // A -0.0 coordinate survives as its own bit pattern.
    assert_eq!(m.nodes[0].map(f32::to_bits), [0x40c0_0000, 0x8000_0000, 0]);
    let f0 = &r.faces[0];
    assert_eq!(f0.vertices, [529, 458, 473]);
    assert_eq!(
        (f0.records[0].time_step, f0.records[0].energy.to_bits()),
        (1, 0x31d9_d948)
    );
    let last = r.faces[933].records.last().unwrap();
    assert_eq!((last.time_step, last.energy.to_bits()), (15, 0x309a_e32a));
}

#[test]
fn dump_grammar_head() {
    let d = csbin::dump(&load(LIB_CUT));
    let lines: Vec<&str> = d.lines().collect();
    assert_eq!(
        &lines[..6],
        &[
            "csbin 3",
            "lengths 44 12 264 16 8",
            "timesteps 5 3dcccccd",
            "recordtype 0",
            "nodes 30",
            "node 00000000 40800000 3fcccccd",
        ]
    );
    assert!(d.contains("\nreceivers 1\nreceiver 3389 Plane\\x20Receiver\nfaces 40\nface 1 0 5\nrecords 5\nrecord 0 338e9944\nrecord 1 30e322b9\n"));
    assert!(d.ends_with("record 4 2948e575\n"));
    // 5 header lines + 30 nodes + receivers, receiver, faces + 40 * (face, records, 5 records).
    assert_eq!(lines.len(), 5 + 30 + 3 + 40 * 7);
    assert_eq!(csbin::dump_file(&fixture(LIB_CUT)), d);
}

// ---- negative cases ----

#[test]
fn missing_file_is_not_found() {
    // Upstream's RSBIN::ImportBIN returns true here, with an uninitialised header.
    let p = fixture("upstream/lib_interface/no_such_file.csbin");
    assert!(matches!(
        csbin::read_file(&p),
        Err(FormatError::NotFound(_))
    ));
    assert_eq!(csbin::dump_file(&p), "error notfound\n");
}

#[test]
fn every_truncation_is_truncated() {
    let b = bytes(LIB_CUT);
    for len in 0..b.len() {
        match csbin::read(&b[..len]) {
            Err(FormatError::Truncated { .. }) => {}
            other => panic!("truncated to {len} bytes: {other:?}"),
        }
    }
    assert!(csbin::read(&b).is_ok());
}

#[test]
fn truncation_of_the_large_fixtures() {
    for rel in [PY_CUT, SPPS] {
        let b = bytes(rel);
        for len in [0, 3, 43, 44, 45, 100, b.len() / 2, b.len() - 8, b.len() - 1] {
            assert!(
                matches!(csbin::read(&b[..len]), Err(FormatError::Truncated { .. })),
                "{rel} truncated to {len}"
            );
        }
    }
}

fn put_i32(b: &mut [u8], at: usize, v: i32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

#[test]
fn unsupported_versions() {
    let b = bytes(LIB_CUT);
    for v in [0, 1, 2, 4, -3, i32::MAX] {
        let mut m = b.clone();
        put_i32(&mut m, 0, v);
        match csbin::read(&m) {
            Err(FormatError::Version { found, .. }) => assert_eq!(found, v.to_string()),
            other => panic!("version {v}: {other:?}"),
        }
    }
    // Only the version field is needed to reject a version.
    assert!(matches!(
        csbin::read(&4i32.to_le_bytes()),
        Err(FormatError::Version { .. })
    ));
    // Through a file too, in a folder removed when the test passes (`common/scratch.rs`).
    let p = common::scratch::fresh("csbin_golden", "version2").join("csbin_golden_version2.csbin");
    let mut m = b.clone();
    put_i32(&mut m, 0, 2);
    std::fs::write(&p, &m).unwrap();
    assert_eq!(csbin::dump_file(&p), "error version\n");
}

fn invalid(b: &[u8]) -> bool {
    matches!(csbin::read(b), Err(FormatError::Invalid(_)))
}

// Offsets in the lib_interface fixture: header 0..44, 30 nodes 44..404, receiver 404..668
// (quantFaces at 408), first face 668..684 (nbRecords at 680), its first value at 684.
#[test]
fn invalid_headers_and_counts() {
    let b = bytes(LIB_CUT);
    let with = |at: usize, v: i32| {
        let mut m = b.clone();
        put_i32(&mut m, at, v);
        m
    };
    // Struct lengths below what their fields need.
    assert!(invalid(&with(4, 43)));
    assert!(invalid(&with(8, 11)));
    assert!(invalid(&with(12, 262)));
    assert!(invalid(&with(16, 15)));
    assert!(invalid(&with(20, 7)));
    // Negative counts.
    assert!(invalid(&with(24, -1)));
    assert!(invalid(&with(28, -1)));
    assert!(invalid(&with(32, -5)));
    assert!(invalid(&with(408, -40)));
    assert!(invalid(&with(680, -5)));
    // Unknown record type.
    assert!(invalid(&with(40, 10)));
    assert!(invalid(&with(40, -1)));
    // Face vertices outside the 30 nodes.
    assert!(invalid(&with(668, 30)));
    assert!(invalid(&with(672, -1)));
    // A time step at or beyond nbTimeStep (5).
    let mut m = b.clone();
    m[684..686].copy_from_slice(&5u16.to_le_bytes());
    assert!(invalid(&m));
    // Counts larger than the data are truncation, not an allocation.
    assert!(matches!(
        csbin::read(&with(24, i32::MAX)),
        Err(FormatError::Truncated { .. })
    ));
    assert!(matches!(
        csbin::read(&with(28, i32::MAX)),
        Err(FormatError::Truncated { .. })
    ));
    assert!(matches!(
        csbin::read(&with(408, i32::MAX)),
        Err(FormatError::Truncated { .. })
    ));
    assert!(matches!(
        csbin::read(&with(680, i32::MAX)),
        Err(FormatError::Truncated { .. })
    ));
}

#[test]
fn valid_edge_values() {
    let b = bytes(LIB_CUT);
    // All ten record types are accepted.
    for t in 0..10 {
        let mut m = b.clone();
        put_i32(&mut m, 40, t);
        assert_eq!(csbin::read(&m).unwrap().record_type.as_i32(), t);
    }
    // Trailing bytes are ignored, as upstream ignores them.
    let mut m = b.clone();
    m.extend_from_slice(&[0xAB; 13]);
    assert_eq!(csbin::read(&m).unwrap(), load(LIB_CUT));
    // Padding is never read: scribbling over it changes nothing.
    let mut m = b.clone();
    m[667] ^= 0xFF; // t_RecepteurS padding byte
    m[686] ^= 0xFF; // t_faceValue padding
    m[687] ^= 0xFF;
    assert_eq!(csbin::read(&m).unwrap(), load(LIB_CUT));
    // A name with no NUL keeps all 255 bytes.
    let mut m = b.clone();
    m[412..667].fill(b'n');
    assert_eq!(csbin::read(&m).unwrap().receivers[0].name, vec![b'n'; 255]);
    // An empty file body: no nodes, no receivers.
    let mut h = b[..44].to_vec();
    put_i32(&mut h, 24, 0);
    put_i32(&mut h, 28, 0);
    let e = csbin::read(&h).unwrap();
    assert!(e.nodes.is_empty() && e.receivers.is_empty());
    assert_eq!(
        csbin::dump(&e),
        "csbin 3\nlengths 44 12 264 16 8\ntimesteps 5 3dcccccd\nrecordtype 0\nnodes 0\nreceivers 0\n"
    );
}

/// Re-lays the lib_interface fixture with every struct padded to a larger stored length:
/// the reader must step by the stored lengths and read the same model.
#[test]
fn stored_struct_lengths_are_honoured() {
    let b = bytes(LIB_CUT);
    let orig = load(LIB_CUT);
    let (hx, nx, rx, fx, vx) = (8usize, 4usize, 12usize, 4usize, 8usize);
    let mut out = Vec::new();
    let mut hdr = b[..44].to_vec();
    put_i32(&mut hdr, 4, (44 + hx) as i32);
    put_i32(&mut hdr, 8, (12 + nx) as i32);
    put_i32(&mut hdr, 12, (264 + rx) as i32);
    put_i32(&mut hdr, 16, (16 + fx) as i32);
    put_i32(&mut hdr, 20, (8 + vx) as i32);
    out.extend_from_slice(&hdr);
    out.extend(std::iter::repeat_n(0xEE, hx));
    let mut pos = 44;
    let mut copy = |len: usize, extra: usize, out: &mut Vec<u8>| {
        out.extend_from_slice(&b[pos..pos + len]);
        out.extend(std::iter::repeat_n(0xEE, extra));
        pos += len;
    };
    for _ in 0..30 {
        copy(12, nx, &mut out);
    }
    copy(264, rx, &mut out);
    for _ in 0..40 {
        copy(16, fx, &mut out);
        for _ in 0..5 {
            copy(8, vx, &mut out);
        }
    }
    let m = csbin::read(&out).unwrap();
    assert_eq!(consumed(&m), out.len());
    assert_eq!(
        m.lengths,
        StructLengths {
            header: 52,
            node: 16,
            receiver: 276,
            face: 20,
            value: 16
        }
    );
    assert_eq!(m.nodes, orig.nodes);
    assert_eq!(m.receivers, orig.receivers);
}
