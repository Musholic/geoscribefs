use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

use geoscribefs::volume_manager::FsBackend;

pub trait TempVolume {
    fn delete(&mut self) -> io::Result<ExitStatus>;
    fn path(&self) -> &Path;
    fn volume_name(&self) -> &str {
        self.path()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
    }

    fn park_path(&self, pid: u32) -> PathBuf {
        Path::new("/tmp/geoscribefs")
            .join(format!("{:04}", pid))
            .join(".volumes")
            .join(self.volume_name())
    }
}

pub struct TempVolumeGuard {
    pub volume: Box<dyn TempVolume>,
}

impl Drop for TempVolumeGuard {
    fn drop(&mut self) {
        tracing::debug!(path = ?self.volume.path(), "Dropping temp volume");
        if let Err(e) = self.volume.delete() {
            tracing::error!(
                error = %e,
                path = ?self.volume.path(),
                "Failed to delete temp volume"
            )
        }
    }
}

pub fn create_temp_volume(backend: FsBackend) -> TempVolumeGuard {
    let volume_name = format!(
        "vol_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    create_temp_volume_named(backend, &volume_name)
}

pub fn create_temp_volume_named(backend: FsBackend, volume_name: &str) -> TempVolumeGuard {
    let volume: Box<dyn TempVolume> = match backend {
        FsBackend::Btrfs => Box::new(BtrfsTempVolume::create(volume_name)),
        FsBackend::Zfs => Box::new(ZfsTempVolume::create(volume_name)),
    };
    TempVolumeGuard { volume }
}

pub struct BtrfsTempVolume {
    pub volume_path: PathBuf,
}

impl BtrfsTempVolume {
    fn create(volume_name: &str) -> Self {
        let random_dir = format!("{:x}", rand::random::<u64>());
        let base_volume_path = Path::new("/var/lib/geoscribefs/btrfs").join(random_dir);
        std::fs::create_dir_all(&base_volume_path).expect("Failed to create base volume directory");
        let volume_path = base_volume_path.join(volume_name);
        let status = Command::new("btrfs")
            .args(["subvolume", "create", volume_path.to_str().unwrap()])
            .status()
            .expect("Failed to create subvolume");

        assert!(
            status.success(),
            "Failed to create subvolume with path {}: {}",
            volume_path.to_string_lossy(),
            status
        );

        Self { volume_path }
    }
}

impl TempVolume for BtrfsTempVolume {
    fn delete(&mut self) -> io::Result<ExitStatus> {
        Command::new("btrfs")
            .args(["subvolume", "delete", self.volume_path.to_str().unwrap()])
            .status()
    }

    fn path(&self) -> &Path {
        &self.volume_path
    }
}

pub struct ZfsTempVolume {
    pub volume_path: PathBuf,
    parent_dataset: String,
}

impl ZfsTempVolume {
    fn create(volume_name: &str) -> Self {
        let zpool_name = "zpool_geoscribefs".to_string();
        let random_parent_dataset = format!("{:x}", rand::random::<u64>());
        let parent_dataset = format!("{}/{}", zpool_name, random_parent_dataset);
        let dataset = format!("{}/{}", parent_dataset, volume_name);
        Command::new("zfs")
            .args(["create", "-p", &dataset])
            .status()
            .expect("Failed to create ZFS volume");

        let volume_path = format!("/{}", dataset);

        Self {
            volume_path: PathBuf::from(volume_path),
            parent_dataset,
        }
    }
}

impl TempVolume for ZfsTempVolume {
    fn delete(&mut self) -> io::Result<ExitStatus> {
        Command::new("zfs")
            .args(["destroy", "-r", self.parent_dataset.as_str()])
            .status()
    }

    fn path(&self) -> &Path {
        &self.volume_path
    }
}
