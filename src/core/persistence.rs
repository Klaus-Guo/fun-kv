#[cfg(unix)]
use std::sync::Arc;
use std::{
    cmp::min,
    fs::{File, OpenOptions},
    io::{self, Read, Seek},
    sync::atomic::Ordering,
};

#[cfg(unix)]
use parking_lot::RwLock;

#[cfg(unix)]
use crate::storage::io::DiskIO;
use crate::{
    constants::*,
    core::record::Record,
    error::{DbError, Result},
    storage::{format::get_format, metadata::Metadata},
};

use super::FunKV;

impl FunKV {
    pub fn flush_all(&self) {
        if self.persistency {
            if let Some(ref write_buffer) = self.write_buffer {
                let _ = write_buffer.force_flush();
            }

            if let Some(ref disk_io) = self.disk_io {
                let mut metadata = self._metadata.write();
                metadata.total_records = self.stats.record_count.load(Ordering::Relaxed) as u64;
                metadata.total_size = self.stats.disk_usage.load(Ordering::Relaxed);
                metadata.fragmentation = self.free_space.read().get_fragmentation_percent();
                metadata.update();

                let _ = disk_io.write().write_metadata(metadata.as_bytes());
                let _ = disk_io.write().flush();
            }
        }
    }

    pub(super) fn load_value_from_disk(&self, record: &Record) -> Result<Vec<u8>> {
        let sector = record.sector.load(Ordering::Acquire);

        if !self.persistency || sector == 0 {
            return Err(DbError::InvalidRecord);
        }

        let metadata_version = self._metadata.read().version;
        let format = get_format(metadata_version);

        let total_size = format.total_size(record.key.len(), record.value_len);
        let sectors_needed = total_size.div_ceil(BLOCK_SIZE);

        let disk_io = self
            .disk_io
            .as_ref()
            .ok_or_else(|| {
                DbError::IoError(io::Error::new(
                    io::ErrorKind::NotFound,
                    "No disk IO available",
                ))
            })?
            .read();

        let data = disk_io.read_sectors_sync(sector, sectors_needed as u64)?;

        let offset = format.record_header_size(record.key.len());
        if offset + record.value_len > data.len() {
            return Err(DbError::InvalidRecord);
        }

        Ok(data[offset..offset + record.value_len].to_vec())
    }

    pub(super) fn open_device(
        &mut self,
        file_path: &Option<String>,
        file_size: Option<u64>,
    ) -> Result<()> {
        if let Some(path) = file_path {
            #[cfg(target_os = "linux")]
            use std::os::unix::fs::OpenOptionsExt;
            #[cfg(unix)]
            use std::path::Path;

            #[cfg(unix)]
            let (file, use_direct_io) = if Path::new("/.dockerenv").exists() {
                let file = FunKV::regular_open(path)?;
                (file, false) // Don't use O_DIRECT in Docker
            } else {
                #[cfg(target_os = "linux")]
                {
                    match OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .custom_flags(libc::O_DIRECT)
                        .open(path)
                    {
                        Ok(file) => (file, true),
                        Err(_) => {
                            let file = FunKV::regular_open(path)?;

                            (file, false)
                        }
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    let file = FunKV::regular_open(path)?;

                    (file, false)
                }
            };

            #[cfg(not(unix))]
            let file = FunKV::regular_open(path)?;

            let metadata = file.metadata().map_err(DbError::IoError)?;
            self.persistence_size = metadata.len();

            let was_newly_created = self.persistence_size == 0;

            if was_newly_created {
                let target_size = file_size.unwrap_or(DEFAULT_PERSISTENT_SIZE as u64);
                file.set_len(target_size).map_err(DbError::IoError)?;
                self.persistence_size = target_size;

                self.free_space.write().initialize(self.persistence_size)?;

                let mut metadata = self._metadata.write();
                metadata.persistent_size = self.persistence_size;
                metadata.update();
            } else {
                let is_empty_file = {
                    let mut temp_file = file.try_clone().map_err(DbError::IoError)?;
                    temp_file
                        .metadata()
                        .map(|m| {
                            if m.len() > 0 {
                                let mut buffer = vec![0u8; min(READ_BUFFER_SIZE, m.len() as usize)];
                                temp_file.seek(std::io::SeekFrom::Start(0)).ok();
                                temp_file.read_exact(&mut buffer).ok();
                                buffer.iter().all(|&b| b == 0)
                            } else {
                                false
                            }
                        })
                        .unwrap_or(false)
                };

                if is_empty_file {
                    self.free_space.write().initialize(self.persistence_size)?;
                } else {
                    self.free_space
                        .write()
                        .set_persistence_size(self.persistence_size);
                }
            }

            #[cfg(unix)]
            {
                use std::os::unix::io::AsRawFd;

                let file_arc = Arc::new(file);
                let fd = file_arc.as_raw_fd();
                self.file_fd = Some(fd);

                self.persistence_file =
                    Some(file_arc.as_ref().try_clone().map_err(DbError::IoError)?);
                self.disk_io = Some(Arc::new(RwLock::new(DiskIO::new(file_arc, use_direct_io)?)));
            }

            #[cfg(not(unix))]
            {
                self.persistence_file = Some(file.as_ref().try_clone().map_err(DbError::IoError)?);
                self.disk_io = Some(Arc::new(RwLock::new(DiskIO::new(file, use_direct_io)?)));
            }

            if !was_newly_created {
                let disk_io = self.disk_io.as_ref().unwrap().read();
                if let Ok(metadata_byte) = disk_io.read_metadata() {
                    if let Some(loaded_metadata) = Metadata::from_bytes(&metadata_byte) {
                        self.stats
                            .disk_usage
                            .store(loaded_metadata.total_size, Ordering::Relaxed);
                        *self._metadata.write() = loaded_metadata;
                    }
                }
            }
        }

        Ok(())
    }

    fn regular_open(path: &String) -> Result<File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(DbError::IoError)
    }
}
