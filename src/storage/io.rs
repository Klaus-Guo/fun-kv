#[cfg(target_os = "linux")]
use io_uring::{IoUring, Probe, opcode, types};
use std::fs::File;
#[cfg(unix)]
use std::io;
#[cfg(unix)]
use std::os::unix::io::RawFd;
use std::sync::Arc;

use crate::constants::*;
use crate::error::{DbError, Result};
#[cfg(unix)]
use crate::utils::allocator::AlignedBuffer;

pub struct DiskIO {
    #[cfg(target_os = "linux")]
    ring: Option<IoUring>,
    _file: Arc<File>,
    #[cfg(unix)]
    fd: RawFd,
    _use_direct_io: bool,
}

impl DiskIO {
    #[cfg(unix)]
    pub fn new(file: Arc<File>, use_direct_io: bool) -> Result<Self> {
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();

        #[cfg(target_os = "linux")]
        {
            let ring: Option<IoUring> = IoUring::builder()
                .setup_sqpoll(IOURING_SQPOLL_IDLE_MS)
                .build(IOURING_QUEUE_SIZE)
                .ok();

            if let Some(ref r) = ring {
                let mut probe = Probe::new();
                if r.submitter().register_probe(&mut probe).is_ok()
                    && probe.is_supported(opcode::Read::CODE)
                    && probe.is_supported(opcode::Write::CODE)
                {
                    return Ok(Self {
                        ring,
                        _file: file.clone(),
                        fd,
                        _use_direct_io: use_direct_io,
                    });
                }
            }

            Ok(Self {
                ring,
                _file: file,
                fd,
                _use_direct_io: false,
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = use_direct_io;
            Ok(Self {
                _file: file,
                fd,
                _use_direct_io: false,
            })
        }
    }

    #[cfg(not(unix))]
    pub fn new_from_file(file: File) -> Result<Self> {
        Ok(Self {
            _file: Arc::new(file),
            _use_direct_io: false,
        })
    }

    pub fn read_sectors_sync(&self, sector: u64, count: u64) -> Result<Vec<u8>> {
        let size = (count * BLOCK_SIZE as u64) as usize;
        let offset = sector * BLOCK_SIZE as u64;

        #[cfg(unix)]
        {
            if self._use_direct_io {
                let mut buffer = AlignedBuffer::new(size)?;
                buffer.set_len(size);

                unsafe {
                    self.do_pread(
                        buffer.as_mut_ptr() as *mut libc::c_void,
                        size,
                        offset as libc::off_t,
                    )?;
                }

                Ok(buffer.as_slice().to_vec())
            } else {
                let mut buffer = vec![0u8; size];

                unsafe {
                    self.do_pread(
                        buffer.as_mut_ptr() as *mut libc::c_void,
                        size,
                        offset as libc::off_t,
                    )?;
                }

                Ok(buffer)
            }
        }

        #[cfg(not(unix))]
        {
            let mut buffer = vec![0u8, size];

            #[cfg(target_os = "windows")]
            {
                use std::os::windows::fs::FileExt;
                self._file
                    .seek_read(&mut buffer, offset)
                    .map_err(DbError::IoError)?;
            }

            #[cfg(not(any(unix, target_os = "windows")))]
            {
                use std::io::{Read, Seek, SeekFrom};

                let mut file = self._file.as_ref().try_clone().map_err(DbError::IoError)?;

                file.seek(SeekFrom::Start(offset))
                    .map_err(DbError::IoError)?;

                file.read_exact(&mut buffer).map_err(DbError::IoError)?;
            }

            Ok(buffer)
        }
    }

    pub fn write_sectors_sync(&self, sector: u64, data: &[u8]) -> Result<()> {
        let offset = sector * BLOCK_SIZE as u64;

        #[cfg(unix)]
        {
            let written = if self._use_direct_io {
                let mut aligned_buffer = AlignedBuffer::new(data.len())?;
                aligned_buffer.set_len(data.len());
                aligned_buffer.as_mut_slice().copy_from_slice(data);

                unsafe {
                    libc::pwrite(
                        self.fd,
                        aligned_buffer.as_ptr() as *const libc::c_void,
                        aligned_buffer.len(),
                        offset as libc::off_t,
                    )
                }
            } else {
                unsafe {
                    libc::pwrite(
                        self.fd,
                        data.as_ptr() as *const libc::c_void,
                        data.len(),
                        offset as libc::off_t,
                    )
                }
            };

            if written < 0 {
                return Err(DbError::IoError(io::Error::last_os_error()));
            }

            if written as usize != data.len() {
                return Err(DbError::IoError(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Partial write",
                )));
            }
        }

        #[cfg(not(unix))]
        {
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::fs::FileExt;
                self._file
                    .seek_write(data, offset)
                    .map_err(FeoxError::IoError)?;
            }

            #[cfg(not(any(unix, target_os = "windows")))]
            {
                use std::io::{Seek, SeekFrom, Write};

                let mut file = self._file.as_ref().try_clone().map_err(DbError::IoError)?;

                file.seek(SeekFrom::Start(offset))
                    .map_err(DbError::IoError)?;

                file.write_all(data).map_err(DbError::IoError)?;

                // Ensure data is written to disk
                file.sync_data().map_err(DbError::IoError)?;
            }
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn batch_write(&mut self, writes: Vec<(u64, Vec<u8>)>) -> Result<()> {
        if let Some(ref mut ring) = self.ring {
            for chunk in writes.chunks(IOURING_MAX_BATCH) {
                let mut aligned_buffer = Vec::new();

                for (_sector, data) in chunk {
                    let mut aligned = AlignedBuffer::new(data.len())?;
                    aligned.set_len(data.len());
                    aligned.as_mut_slice().copy_from_slice(data);
                    aligned_buffer.push(aligned);
                }

                unsafe {
                    let mut sq = ring.submission();

                    for (i, (sector, _)) in chunk.iter().enumerate() {
                        let offset = sector * BLOCK_SIZE as u64;
                        let buffer = &aligned_buffer[i];

                        let write_e = opcode::Write::new(
                            types::Fd(self.fd),
                            buffer.as_ptr(),
                            buffer.len() as u32,
                        )
                        .offset(offset)
                        .build()
                        .user_data(i as u64);

                        sq.push(&write_e)
                            .map_err(|_| DbError::IoError(io::Error::other("SQ full")))?;
                    }
                }

                let submitted = ring
                    .submit_and_wait(chunk.len())
                    .map_err(DbError::IoError)?;

                let mut completed = 0;
                for cqe in ring.completion() {
                    if cqe.result() < 0 {
                        return Err(DbError::IoError(io::Error::from_raw_os_error(
                            -cqe.result(),
                        )));
                    }

                    completed += 1;
                    if completed >= submitted {
                        break;
                    }
                }
            }

            self.flush()?;

            Ok(())
        } else {
            for (sector, data) in writes {
                self.write_sectors_sync(sector, &data)?;
            }
            Ok(())
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn batch_write(&mut self, writes: Vec<(u64, Vec<u8>)>) -> Result<()> {
        for (sector, data) in writes {
            self.write_sectors_sync(sector, &data)?;
        }
        Ok(())
    }

    pub fn read_metadata(&self) -> Result<Vec<u8>> {
        self.read_sectors_sync(METADATA_BLOCK, 1)
    }

    pub fn write_metadata(&self, metadata: &[u8]) -> Result<()> {
        if metadata.len() > BLOCK_SIZE {
            return Err(DbError::InvalidValueSize);
        }

        let mut block_data = vec![0u8; BLOCK_SIZE];
        block_data[..metadata.len()].copy_from_slice(metadata);

        self.write_sectors_sync(METADATA_BLOCK, &block_data)?;
        self.flush()
    }

    pub fn flush(&self) -> Result<()> {
        #[cfg(unix)]
        unsafe {
            if libc::fsync(self.fd) == -1 {
                return Err(DbError::IoError(io::Error::last_os_error()));
            }
        }

        #[cfg(not(unix))]
        {
            self._file.sync_all().map_err(DbError::IoError)?;
        }

        Ok(())
    }

    pub fn shutdown(&mut self) {
        #[cfg(target_os = "linux")]
        {
            if let Some(ref mut ring) = self.ring {
                if ring.submit_and_wait(0).is_ok() {
                    while ring.completion().next().is_some() {}
                }
            }
            self.ring = None;
        }
    }

    #[cfg(unix)]
    unsafe fn do_pread(
        &self,
        buf: *mut libc::c_void,
        size: usize,
        offset: libc::off_t,
    ) -> Result<()> {
        let ret = libc::pread(self.fd, buf, size, offset);
        if ret < 0 {
            return Err(DbError::IoError(io::Error::last_os_error()));
        }
        if ret as usize != size {
            return Err(DbError::IoError(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("Read {} bytes, expected {}", ret, size),
            )));
        }
        Ok(())
    }
}
