//! Particle trajectories (`.pbin`), written by SPPS when `nbparticules_rendu` is above zero, and
//! by upstream's `particleio::ParticuleIO`. Read only. Spec: `docs/formats/pbin.md`.
//!
//! Upstream declares every header field as `unsigned long` (`bpLong`), so the layout depends on the
//! platform that wrote the file. This reader reads the Windows (LLP64) layout, where `unsigned long`
//! is 4 bytes: a 28-byte file header, an 8-byte particle header (`u32` step count, `u16` first
//! step, 2 bytes of padding) and 16-byte steps. A file written on an LP64 platform (Linux, macOS)
//! has 8-byte fields; read with this layout its version field is the zero high half of its
//! particle count, so it is refused with [`FormatError::Version`].
//!
//! Upstream's reader checks nothing: a short read leaves zeros or uninitialised values in what it
//! hands out and reports success. This reader instead requires the header's struct sizes to be
//! this layout's (28/16/8) and the file to be exactly as long as its headers declare, and says
//! why when it is not.

use std::path::Path;

use super::{Cursor, FormatError, Result, f32_hex};

/// `PARTICLE_BINARY_VERSION_INFORMATION`, the only version upstream writes.
pub const FORMAT_VERSION: u32 = 1;
/// `sizeof(binaryFHeader)` on Windows: six 4-byte `unsigned long` and one `float`.
pub const FILE_HEADER_SIZE: usize = 28;
/// `sizeof(binaryPHeader)` on Windows: a 4-byte `unsigned long`, an `unsigned short`, 2 bytes of
/// padding.
pub const PARTICLE_HEADER_SIZE: usize = 8;
/// `sizeof(binaryPTimeStep)`: a `vec3` of `float` and a `float`.
pub const TIME_STEP_SIZE: usize = 16;

/// The file header, upstream's `binaryFHeader`, in its field order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileHeader {
    /// `nbParticles`: how many particle records follow.
    pub nb_particles: u32,
    /// `formatVersion`: always [`FORMAT_VERSION`] in a file this reader accepts.
    pub format_version: u32,
    /// `fileInfoLength`: the writer's `sizeof(binaryFHeader)`, always [`FILE_HEADER_SIZE`] in a
    /// file this reader accepts. Upstream's reader stores it and never uses it.
    pub file_info_length: u32,
    /// `particleInfoLength`: the writer's `sizeof(binaryPTimeStep)`, always [`TIME_STEP_SIZE`]
    /// here. Upstream's reader uses it as a stride for files whose version is not 1 and to skip
    /// the rest of a partly read particle.
    pub particle_info_length: u32,
    /// `particleHeaderInfoLength`: the writer's `sizeof(binaryPHeader)`, always
    /// [`PARTICLE_HEADER_SIZE`] here. Upstream's reader uses it like `particle_info_length`, and
    /// also after every particle that has no steps.
    pub particle_header_info_length: u32,
    /// `nbTimeStepMax`. SPPS writes the simulation's time-step count here; `ParticuleIO` writes
    /// the largest particle step count. Neither is checked against the data.
    pub nb_time_step_max: u32,
    /// `timeStep`: the simulation time step in seconds.
    pub time_step: f32,
}

/// One particle's record header, upstream's `binaryPHeader`, without its padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticleHeader {
    /// `nbTimeStep`: how many [`TimeStep`] records follow this header.
    pub nb_time_step: u32,
    /// `firstTimeStep`: the simulation step of the particle's first record. SPPS stores an `int`
    /// here, so a first step above 65535 has already wrapped in the file.
    pub first_time_step: u16,
}

/// One recorded position, upstream's `binaryPTimeStep`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeStep {
    /// `position`: x, y, z in metres.
    pub position: [f32; 3],
    /// `energy`: the particle's energy at this step.
    pub energy: f32,
}

/// A whole `.pbin` file.
///
/// Particle headers and steps are kept in two flat vectors, in file order, so the model costs no
/// more memory than the file: particle `i` owns the `particles[i].nb_time_step` steps that follow
/// those of particles `0..i`. [`ParticleFile::iter`] pairs them up.
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleFile {
    pub header: FileHeader,
    pub particles: Vec<ParticleHeader>,
    pub steps: Vec<TimeStep>,
}

impl ParticleFile {
    /// Each particle's header with its steps, in file order. On a hand-built value whose counts
    /// do not add up, a particle's slice stops where `steps` does.
    pub fn iter(&self) -> impl Iterator<Item = (&ParticleHeader, &[TimeStep])> {
        let mut start = 0usize;
        self.particles.iter().map(move |p| {
            let from = start.min(self.steps.len());
            let to = from
                .saturating_add(p.nb_time_step as usize)
                .min(self.steps.len());
            start = start.saturating_add(p.nb_time_step as usize);
            (p, &self.steps[from..to])
        })
    }

    /// Size in bytes of the file this model reads from or describes.
    pub fn byte_len(&self) -> usize {
        FILE_HEADER_SIZE
            + self.particles.len() * PARTICLE_HEADER_SIZE
            + self.steps.len() * TIME_STEP_SIZE
    }
}

fn read_header(c: &mut Cursor<'_>) -> Result<FileHeader> {
    Ok(FileHeader {
        nb_particles: c.u32()?,
        format_version: c.u32()?,
        file_info_length: c.u32()?,
        particle_info_length: c.u32()?,
        particle_header_info_length: c.u32()?,
        nb_time_step_max: c.u32()?,
        time_step: c.f32()?,
    })
}

fn read_particle_header(c: &mut Cursor<'_>) -> Result<ParticleHeader> {
    let nb_time_step = c.u32()?;
    let first_time_step = c.u16()?;
    c.skip(2)?; // struct padding: uninitialised in SPPS output, never read
    Ok(ParticleHeader {
        nb_time_step,
        first_time_step,
    })
}

/// Parses a whole `.pbin` file.
///
/// Errors: [`FormatError::Truncated`] when the data ends before the header, a particle header or
/// a particle's steps; [`FormatError::Version`] when `formatVersion` is not 1;
/// [`FormatError::Invalid`] when the header's struct sizes are not 28/16/8, or when bytes remain
/// after the last particle the header declares.
pub fn read(bytes: &[u8]) -> Result<ParticleFile> {
    let mut c = Cursor::new(bytes, "pbin");
    let header = read_header(&mut c)?;
    if header.format_version != FORMAT_VERSION {
        return Err(FormatError::Version {
            what: "pbin format",
            found: header.format_version.to_string(),
            expected: "1",
        });
    }
    // The writer's struct sizes. Upstream's writers always store their own sizeof() here, so any
    // other value means a different struct layout, which upstream's reader would misread without
    // noticing. It also uses particleHeaderInfoLength as a stride after an empty particle
    // (part_io.cpp:229-233), so accepting other values would make the layout ambiguous.
    let lengths = (
        header.file_info_length,
        header.particle_info_length,
        header.particle_header_info_length,
    );
    if lengths
        != (
            FILE_HEADER_SIZE as u32,
            TIME_STEP_SIZE as u32,
            PARTICLE_HEADER_SIZE as u32,
        )
    {
        return Err(FormatError::Invalid(format!(
            "struct sizes {}/{}/{} (file header/step/particle header) are not the Windows \
             layout's 28/16/8",
            lengths.0, lengths.1, lengths.2
        )));
    }

    // Pass 1: particle headers, each step block checked against the bytes left, so the exact
    // step total is known before anything is reserved for it.
    let n = c.check_count(header.nb_particles as usize, PARTICLE_HEADER_SIZE)?;
    let mut particles = Vec::with_capacity(n);
    let mut total_steps = 0usize;
    for _ in 0..n {
        let p = read_particle_header(&mut c)?;
        let k = c.check_count(p.nb_time_step as usize, TIME_STEP_SIZE)?;
        c.skip(k * TIME_STEP_SIZE)?;
        total_steps += k;
        particles.push(p);
    }
    if !c.is_empty() {
        return Err(FormatError::Invalid(format!(
            "{} bytes after the last of the {} particles the header declares",
            c.remaining(),
            n
        )));
    }

    // Pass 2: the steps, into one vector of exactly the right size.
    let mut steps = Vec::with_capacity(total_steps);
    let mut c = Cursor::new(bytes, "pbin");
    c.seek(FILE_HEADER_SIZE)?;
    for p in &particles {
        c.skip(PARTICLE_HEADER_SIZE)?;
        for _ in 0..p.nb_time_step {
            let position = [c.f32()?, c.f32()?, c.f32()?];
            let energy = c.f32()?;
            steps.push(TimeStep { position, energy });
        }
    }
    Ok(ParticleFile {
        header,
        particles,
        steps,
    })
}

/// Reads and parses a `.pbin` file; a missing file is [`FormatError::NotFound`].
pub fn read_file(path: &Path) -> Result<ParticleFile> {
    read(&super::read_file(path)?)
}

/// The canonical dump (grammar in `docs/formats/pbin.md`).
pub fn dump(value: &ParticleFile) -> String {
    use std::fmt::Write as _;
    let h = &value.header;
    let mut out = String::with_capacity(64 + value.particles.len() * 16 + value.steps.len() * 42);
    let _ = writeln!(out, "pbin {}", h.format_version);
    let _ = writeln!(
        out,
        "header {} {} {} {} {} {} {}",
        h.nb_particles,
        h.format_version,
        h.file_info_length,
        h.particle_info_length,
        h.particle_header_info_length,
        h.nb_time_step_max,
        f32_hex(h.time_step)
    );
    let _ = writeln!(out, "particles {}", value.particles.len());
    for (p, steps) in value.iter() {
        let _ = writeln!(out, "particle {} {}", p.nb_time_step, p.first_time_step);
        for s in steps {
            let _ = writeln!(
                out,
                "step {} {} {} {}",
                f32_hex(s.position[0]),
                f32_hex(s.position[1]),
                f32_hex(s.position[2]),
                f32_hex(s.energy)
            );
        }
    }
    out
}

fn error_kind(e: &FormatError) -> &'static str {
    match e {
        FormatError::NotFound(_) => "notfound",
        FormatError::Truncated { .. } => "truncated",
        FormatError::Version { .. } => "version",
        FormatError::Invalid(_) => "invalid",
        FormatError::Io(_) => "io",
    }
}

/// The canonical dump of a file, or `error <kind>` when it cannot be read.
pub fn dump_file(path: &Path) -> String {
    match read_file(path) {
        Ok(v) => dump(&v),
        Err(e) => format!("error {}\n", error_kind(&e)),
    }
}
