//! Readers for the legacy EverQuest asset formats.
//!
//! EverQuest predates modern asset pipelines, so its data ships in a handful of
//! container and mesh formats that were designed for DirectX 7/8-era hardware:
//!
//! * [`pfs`] - the `S3D`/`PFS` archive (`.s3d`, `.eqg`) that bundles everything.
//! * [`wld`] - the `WLD` "fragment" format used for classic zones, objects and
//!   animated characters.
//! * [`zone`] - the `ZON`/`TER`/`MOD` triplet used for newer `.eqg` zones.
//! * [`texture`] - the `DDS`/`BMP` textures stored inside those archives.
//! * [`mesh`] - baking fragments into GPU-ready triangle soups.
//!
//! Everything here is pure CPU-side decoding with no engine dependencies, so it
//! can be exercised from tests and command line tools as well as the game.

pub mod audio;
pub mod audit;
pub mod binary_regions;
mod bsp_regions;
pub mod collision;
pub mod environment;
pub mod error;
pub mod liquid_regions;
pub mod loader;
pub mod mesh;
pub mod pfs;
pub mod read;
pub mod spell_effects;
pub mod terrain;
pub mod texture;
pub mod wld;
pub mod zone;
pub mod zone_lines;

pub use error::{Error, Result};
pub use loader::{Instance, Light, Scene, SceneObject, load_zone};
pub mod character;
