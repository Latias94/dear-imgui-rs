use std::fs::{File, Metadata};
use std::io;
use std::os::windows::{
    ffi::OsStrExt,
    fs::{MetadataExt, OpenOptionsExt},
    io::AsRawHandle,
};
use std::path::Path;

use windows_sys::Win32::Foundation::{
    ERROR_INVALID_FUNCTION, ERROR_INVALID_PARAMETER, ERROR_NOT_SUPPORTED,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO, FILE_READ_ATTRIBUTES,
    FileIdInfo, GetFileInformationByHandle, GetFileInformationByHandleEx, MoveFileExW,
};

use super::{FsEntry, FsEntryIdentity, StdFileSystem, canonical_entry};

pub(super) fn is_directory_reparse_point(metadata: &Metadata) -> bool {
    // Junctions and other directory reparse points can redirect traversal.
    let flags = FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT;
    metadata.file_attributes() & flags == flags
}

pub(super) fn inspect_move_source(path: &Path) -> io::Result<(FsEntry, Option<FsEntryIdentity>)> {
    let file = File::options()
        .access_mode(FILE_READ_ATTRIBUTES)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let mut id = FILE_ID_INFO::default();
    // SAFETY: id is the writable output structure for FileIdInfo and the handle is live.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&raw mut id).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    };
    let identity = if ok != 0 {
        // ReFS uses the full 128 bits of its file ID.
        FsEntryIdentity {
            volume: id.VolumeSerialNumber,
            file: u128::from_le_bytes(id.FileId.Identifier),
        }
    } else {
        let error = io::Error::last_os_error();
        match error.raw_os_error().map(|code| code as u32) {
            Some(ERROR_INVALID_FUNCTION | ERROR_INVALID_PARAMETER | ERROR_NOT_SUPPORTED) => {
                legacy_file_identity(&file)?
            }
            _ => return Err(error),
        }
    };
    if identity.file == 0 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "filesystem did not supply a file ID",
        ));
    }
    Ok((
        FsEntry::from_metadata(path.to_owned(), file.metadata()?),
        Some(identity),
    ))
}

fn legacy_file_identity(file: &File) -> io::Result<FsEntryIdentity> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the file owns this live handle and info is writable for the call.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(FsEntryIdentity {
        volume: info.dwVolumeSerialNumber.into(),
        file: (u128::from(info.nFileIndexHigh) << 32) | u128::from(info.nFileIndexLow),
    })
}

pub(super) fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    // Canonical parents give Win32 extended-length paths and leave the final link intact.
    let wide_path = |path: &Path| -> io::Result<Vec<u16>> {
        let path = canonical_entry(&StdFileSystem, path)?;
        let mut wide: Vec<_> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        wide.push(0);
        Ok(wide)
    };
    let from = wide_path(from)?;
    let to = wide_path(to)?;
    // SAFETY: both paths are NUL-terminated and live for the call. Zero flags
    // forbid both replacing an occupied name and copying across volumes.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
