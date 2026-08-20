use ruzstd::decoding::StreamingDecoder;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use tar::Archive;
use zed_extension_api as zed;
use zip::ZipArchive;

pub const MOJO_VERSION: &str = "1.0.0";

const COMPLETE_MARKER: &str = ".complete";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PackageContents {
    LspOnly,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PackageArtifact {
    name: &'static str,
    url: &'static str,
    sha256: &'static str,
    contents: PackageContents,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PlatformArtifacts {
    subdir: &'static str,
    executable: &'static str,
    packages: [PackageArtifact; 2],
}

#[derive(Debug, PartialEq, Eq)]
struct PrefixPatch {
    path: PathBuf,
    placeholder: String,
    file_mode: String,
}

pub fn managed_lsp_path() -> zed::Result<String> {
    let (os, architecture) = zed::current_platform();
    let artifacts = artifacts_for(os, architecture)?;
    let install_dir = format!("mojo-lsp-{}-{}", MOJO_VERSION, artifacts.subdir);
    let binary_path = Path::new(&install_dir).join(artifacts.executable);
    let marker_path = Path::new(&install_dir).join(COMPLETE_MARKER);

    if binary_path.is_file() && marker_path.is_file() {
        zed::make_file_executable(&path_string(&binary_path)?)?;
        return absolute_path_string(&binary_path);
    }

    install(&install_dir, &binary_path, artifacts)?;
    absolute_path_string(&binary_path)
}

fn install(install_dir: &str, binary_path: &Path, artifacts: PlatformArtifacts) -> zed::Result<()> {
    let staging_dir = format!("{install_dir}.installing");
    remove_dir_if_present(Path::new(&staging_dir))?;
    fs::create_dir_all(&staging_dir)
        .map_err(|error| format!("failed to create Mojo LSP staging directory: {error}"))?;

    let mut prefix_patches = Vec::new();
    for package in artifacts.packages {
        let archive_path = Path::new(&staging_dir).join(format!("{}.conda", package.name));
        zed::download_file(
            package.url,
            &path_string(&archive_path)?,
            zed::DownloadedFileType::Uncompressed,
        )?;
        verify_sha256(&archive_path, package.sha256)?;
        prefix_patches.extend(extract_conda_package(
            &archive_path,
            Path::new(&staging_dir),
            package.contents,
        )?);
        fs::remove_file(&archive_path)
            .map_err(|error| format!("failed to remove downloaded Mojo archive: {error}"))?;
    }

    let final_dir = Path::new(install_dir);
    remove_dir_if_present(final_dir)?;
    fs::rename(&staging_dir, final_dir)
        .map_err(|error| format!("failed to activate the managed Mojo LSP: {error}"))?;

    let absolute_prefix = std::env::current_dir()
        .map_err(|error| format!("failed to locate the extension working directory: {error}"))?
        .join(final_dir);
    apply_prefix_patches(final_dir, &absolute_prefix, &prefix_patches)?;

    if !binary_path.is_file() {
        return Err(format!(
            "the Mojo package did not contain `{}`",
            binary_path.display()
        ));
    }
    zed::make_file_executable(&path_string(binary_path)?)?;
    File::create(final_dir.join(COMPLETE_MARKER))
        .and_then(|mut marker| marker.write_all(MOJO_VERSION.as_bytes()))
        .map_err(|error| format!("failed to mark the managed Mojo LSP complete: {error}"))?;
    Ok(())
}

fn artifacts_for(os: zed::Os, architecture: zed::Architecture) -> zed::Result<PlatformArtifacts> {
    use zed::{Architecture, Os};

    match (os, architecture) {
        (Os::Mac, Architecture::Aarch64) => Ok(PlatformArtifacts {
            subdir: "osx-arm64",
            executable: "bin/mojo-lsp-server",
            packages: [
                PackageArtifact {
                    name: "mojo",
                    url: "https://conda.modular.com/max/osx-arm64/mojo-1.0.0-release.conda",
                    sha256: "200bd9ace6e06ad2a9b4f4cce9afa3e5f3c00b3d87a6d57206249df2a5568e38",
                    contents: PackageContents::LspOnly,
                },
                PackageArtifact {
                    name: "mojo-compiler",
                    url: "https://conda.modular.com/max/osx-arm64/mojo-compiler-1.0.0-release.conda",
                    sha256: "c52054bc444d851e5c38cc33e790fb011a1080470244aaa59351ac5056d08c59",
                    contents: PackageContents::All,
                },
            ],
        }),
        (Os::Linux, Architecture::X8664) => Ok(PlatformArtifacts {
            subdir: "linux-64",
            executable: "bin/mojo-lsp-server",
            packages: [
                PackageArtifact {
                    name: "mojo",
                    url: "https://conda.modular.com/max/linux-64/mojo-1.0.0-release.conda",
                    sha256: "5778f999b69cf77bd6f07ccaf32d0bafc258fe75206afc8e88919269cebb6e75",
                    contents: PackageContents::LspOnly,
                },
                PackageArtifact {
                    name: "mojo-compiler",
                    url: "https://conda.modular.com/max/linux-64/mojo-compiler-1.0.0-release.conda",
                    sha256: "4394c6146d47ec7794a9a3ed5775ae158f59f83f8e1aed59408b17c4909821b3",
                    contents: PackageContents::All,
                },
            ],
        }),
        (Os::Linux, Architecture::Aarch64) => Ok(PlatformArtifacts {
            subdir: "linux-aarch64",
            executable: "bin/mojo-lsp-server",
            packages: [
                PackageArtifact {
                    name: "mojo",
                    url: "https://conda.modular.com/max/linux-aarch64/mojo-1.0.0-release.conda",
                    sha256: "0f273174733386a584e41489e556c9014a5e975a57bd556d847f252fc9091d76",
                    contents: PackageContents::LspOnly,
                },
                PackageArtifact {
                    name: "mojo-compiler",
                    url: "https://conda.modular.com/max/linux-aarch64/mojo-compiler-1.0.0-release.conda",
                    sha256: "da1772742c54f1f8f7b883e6338b1fe5de4592f2c882220dcceae623cc661e57",
                    contents: PackageContents::All,
                },
            ],
        }),
        (Os::Windows, _) => Err(
            "managed Mojo installation is unavailable on native Windows; use Zed in WSL or configure an existing `mojo-lsp-server`"
                .into(),
        ),
        _ => Err("managed Mojo installation is unavailable for this platform".into()),
    }
}

fn verify_sha256(path: &Path, expected: &str) -> zed::Result<()> {
    let mut file = File::open(path)
        .map_err(|error| format!("failed to open downloaded Mojo archive: {error}"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to hash downloaded Mojo archive: {error}"))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let actual = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual != expected {
        return Err(format!(
            "downloaded Mojo archive failed SHA-256 verification: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn extract_conda_package(
    archive_path: &Path,
    destination: &Path,
    contents: PackageContents,
) -> zed::Result<Vec<PrefixPatch>> {
    let file = File::open(archive_path)
        .map_err(|error| format!("failed to open Mojo .conda archive: {error}"))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("failed to read Mojo .conda archive: {error}"))?;

    let pkg_index = find_conda_member(&mut archive, "pkg-")?;
    extract_package_member(&mut archive, pkg_index, destination, contents)?;

    let info_index = find_conda_member(&mut archive, "info-")?;
    read_prefix_patches(&mut archive, info_index)
}

fn find_conda_member(archive: &mut ZipArchive<File>, prefix: &str) -> zed::Result<usize> {
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("failed to inspect .conda archive: {error}"))?;
        let name = entry.name();
        if name.starts_with(prefix) && name.ends_with(".tar.zst") {
            return Ok(index);
        }
    }
    Err(format!(
        "invalid .conda archive: missing `{prefix}*.tar.zst` member"
    ))
}

fn extract_package_member(
    zip: &mut ZipArchive<File>,
    index: usize,
    destination: &Path,
    contents: PackageContents,
) -> zed::Result<()> {
    let member = zip
        .by_index(index)
        .map_err(|error| format!("failed to open .conda package payload: {error}"))?;
    let decoder = StreamingDecoder::new(member)
        .map_err(|error| format!("failed to decode .conda package payload: {error}"))?;
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|error| format!("failed to read .conda package payload: {error}"))?;
    let mut found_lsp = false;

    for entry in entries {
        let mut entry =
            entry.map_err(|error| format!("failed to read .conda package entry: {error}"))?;
        let path = entry
            .path()
            .map_err(|error| format!("invalid path in .conda package: {error}"))?
            .into_owned();
        validate_relative_path(&path)?;
        let extract = match contents {
            PackageContents::All => true,
            PackageContents::LspOnly => path == Path::new("bin/mojo-lsp-server"),
        };
        if !extract {
            continue;
        }
        if path == Path::new("bin/mojo-lsp-server") {
            found_lsp = true;
        }
        let unpacked = entry
            .unpack_in(destination)
            .map_err(|error| format!("failed to extract `{}`: {error}", path.display()))?;
        if !unpacked {
            return Err(format!(
                "refused to extract unsafe path `{}` from .conda package",
                path.display()
            ));
        }
    }

    if contents == PackageContents::LspOnly && !found_lsp {
        return Err("Mojo package did not contain `bin/mojo-lsp-server`".into());
    }
    Ok(())
}

fn read_prefix_patches(zip: &mut ZipArchive<File>, index: usize) -> zed::Result<Vec<PrefixPatch>> {
    let member = zip
        .by_index(index)
        .map_err(|error| format!("failed to open .conda metadata payload: {error}"))?;
    let decoder = StreamingDecoder::new(member)
        .map_err(|error| format!("failed to decode .conda metadata payload: {error}"))?;
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|error| format!("failed to read .conda metadata payload: {error}"))?;

    for entry in entries {
        let mut entry =
            entry.map_err(|error| format!("failed to read .conda metadata entry: {error}"))?;
        let path = entry
            .path()
            .map_err(|error| format!("invalid metadata path in .conda package: {error}"))?;
        if path.as_ref() != Path::new("info/paths.json") {
            continue;
        }
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|error| format!("failed to read .conda prefix metadata: {error}"))?;
        return parse_prefix_patches(&data);
    }
    Err("invalid .conda archive: missing `info/paths.json`".into())
}

fn parse_prefix_patches(data: &[u8]) -> zed::Result<Vec<PrefixPatch>> {
    let metadata: zed::serde_json::Value = zed::serde_json::from_slice(data)
        .map_err(|error| format!("failed to parse .conda prefix metadata: {error}"))?;
    let paths = metadata
        .get("paths")
        .and_then(|paths| paths.as_array())
        .ok_or_else(|| "invalid .conda prefix metadata: missing `paths` array".to_string())?;

    let mut patches = Vec::new();
    for path in paths {
        let Some(placeholder) = path
            .get("prefix_placeholder")
            .and_then(|value| value.as_str())
        else {
            continue;
        };
        let path_name = path
            .get("_path")
            .and_then(|value| value.as_str())
            .ok_or_else(|| "invalid .conda prefix metadata: missing `_path`".to_string())?;
        let file_mode = path
            .get("file_mode")
            .and_then(|value| value.as_str())
            .ok_or_else(|| "invalid .conda prefix metadata: missing `file_mode`".to_string())?;
        let path = PathBuf::from(path_name);
        validate_relative_path(&path)?;
        patches.push(PrefixPatch {
            path,
            placeholder: placeholder.into(),
            file_mode: file_mode.into(),
        });
    }
    Ok(patches)
}

fn apply_prefix_patches(
    destination: &Path,
    absolute_prefix: &Path,
    patches: &[PrefixPatch],
) -> zed::Result<()> {
    let replacement = absolute_prefix.to_string_lossy();
    for patch in patches {
        let path = destination.join(&patch.path);
        if !path.is_file() {
            continue;
        }
        if patch.file_mode != "text" {
            return Err(format!(
                "unsupported binary prefix relocation for `{}`",
                patch.path.display()
            ));
        }
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read `{}`: {error}", path.display()))?;
        if !source.contains(&patch.placeholder) {
            return Err(format!(
                "prefix placeholder was not found in `{}`",
                patch.path.display()
            ));
        }
        fs::write(&path, source.replace(&patch.placeholder, &replacement))
            .map_err(|error| format!("failed to relocate `{}`: {error}", path.display()))?;
    }
    Ok(())
}

fn validate_relative_path(path: &Path) -> zed::Result<()> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "unsafe path `{}` in .conda package",
            path.display()
        ));
    }
    Ok(())
}

fn remove_dir_if_present(path: &Path) -> zed::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("failed to remove `{}`: {error}", path.display())),
    }
}

fn path_string(path: &Path) -> zed::Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("path is not valid UTF-8: `{}`", path.display()))
}

fn absolute_path_string(path: &Path) -> zed::Result<String> {
    let absolute = std::env::current_dir()
        .map_err(|error| format!("failed to locate the extension working directory: {error}"))?
        .join(path);
    path_string(&absolute)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Cursor};
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn write_conda_fixture(path: &Path) {
        let package_tar = tar_fixture(&[
            ("bin/mojo-lsp-server", b"lsp"),
            ("bin/unrelated-debugger", b"debugger"),
        ]);
        let info_tar = tar_fixture(&[(
            "info/paths.json",
            br#"{"paths":[{"_path":"share/max/modular.cfg","path_type":"hardlink","file_mode":"text","prefix_placeholder":"/old/prefix"}]}"#,
        )]);
        let package_zstd = zstd::stream::encode_all(Cursor::new(package_tar), 1).unwrap();
        let info_zstd = zstd::stream::encode_all(Cursor::new(info_tar), 1).unwrap();

        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("metadata.json", options).unwrap();
        zip.write_all(br#"{"conda_pkg_format_version":2}"#).unwrap();
        zip.start_file("pkg-fixture.tar.zst", options).unwrap();
        zip.write_all(&package_zstd).unwrap();
        zip.start_file("info-fixture.tar.zst", options).unwrap();
        zip.write_all(&info_zstd).unwrap();
        zip.finish().unwrap();
    }

    fn tar_fixture(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut output = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut output);
            for (path, contents) in files {
                let mut header = tar::Header::new_gnu();
                header.set_size(contents.len() as u64);
                header.set_mode(0o755);
                header.set_cksum();
                builder
                    .append_data(&mut header, path, Cursor::new(contents))
                    .unwrap();
            }
            builder.finish().unwrap();
        }
        output
    }

    #[test]
    fn extracts_only_the_lsp_and_reads_prefix_metadata() {
        let temp = tempdir().unwrap();
        let archive = temp.path().join("fixture.conda");
        let destination = temp.path().join("installed");
        fs::create_dir(&destination).unwrap();
        write_conda_fixture(&archive);

        let patches =
            extract_conda_package(&archive, &destination, PackageContents::LspOnly).unwrap();

        assert_eq!(
            fs::read(destination.join("bin/mojo-lsp-server")).unwrap(),
            b"lsp"
        );
        assert!(!destination.join("bin/unrelated-debugger").exists());
        assert_eq!(
            patches,
            vec![PrefixPatch {
                path: PathBuf::from("share/max/modular.cfg"),
                placeholder: "/old/prefix".into(),
                file_mode: "text".into(),
            }]
        );
    }

    #[test]
    fn applies_text_prefix_relocation() {
        let temp = tempdir().unwrap();
        let config = temp.path().join("share/max/modular.cfg");
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(&config, "package_root = /old/prefix\n").unwrap();
        let patches = vec![PrefixPatch {
            path: PathBuf::from("share/max/modular.cfg"),
            placeholder: "/old/prefix".into(),
            file_mode: "text".into(),
        }];

        apply_prefix_patches(temp.path(), Path::new("/new/prefix"), &patches).unwrap();

        assert_eq!(
            fs::read_to_string(config).unwrap(),
            "package_root = /new/prefix\n"
        );
    }

    #[test]
    fn rejects_a_bad_download_hash() {
        let temp = tempdir().unwrap();
        let archive = temp.path().join("download");
        fs::write(&archive, b"not the expected archive").unwrap();

        let error = verify_sha256(&archive, &"0".repeat(64)).unwrap_err();

        assert!(error.contains("failed SHA-256 verification"));
    }

    #[test]
    fn maps_supported_platforms_and_rejects_native_windows() {
        assert_eq!(
            artifacts_for(zed::Os::Mac, zed::Architecture::Aarch64)
                .unwrap()
                .subdir,
            "osx-arm64"
        );
        assert_eq!(
            artifacts_for(zed::Os::Linux, zed::Architecture::X8664)
                .unwrap()
                .subdir,
            "linux-64"
        );
        assert_eq!(
            artifacts_for(zed::Os::Linux, zed::Architecture::Aarch64)
                .unwrap()
                .subdir,
            "linux-aarch64"
        );
        assert!(artifacts_for(zed::Os::Windows, zed::Architecture::X8664).is_err());
    }

    #[test]
    #[ignore = "requires the real Modular package archives"]
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn installs_and_runs_the_real_macos_lsp_packages() {
        let mojo_archive =
            PathBuf::from(std::env::var("MOJO_CONDA_ARCHIVE").expect("set MOJO_CONDA_ARCHIVE"));
        let compiler_archive = PathBuf::from(
            std::env::var("MOJO_COMPILER_CONDA_ARCHIVE").expect("set MOJO_COMPILER_CONDA_ARCHIVE"),
        );
        let artifacts = artifacts_for(zed::Os::Mac, zed::Architecture::Aarch64).unwrap();
        let temp = tempdir().unwrap();
        let destination = temp.path().join("mojo");
        fs::create_dir(&destination).unwrap();

        let mut patches = Vec::new();
        for (archive, package) in [mojo_archive, compiler_archive]
            .iter()
            .zip(artifacts.packages)
        {
            verify_sha256(archive, package.sha256).unwrap();
            patches.extend(extract_conda_package(archive, &destination, package.contents).unwrap());
        }
        apply_prefix_patches(&destination, &destination, &patches).unwrap();

        let binary = destination.join(artifacts.executable);
        let output = std::process::Command::new(&binary)
            .arg("--version")
            .env("MODULAR_TELEMETRY_ENABLED", "0")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let version_output = [output.stdout, output.stderr].concat();
        assert!(String::from_utf8_lossy(&version_output).contains("LLVM version"));

        let mut child = std::process::Command::new(&binary)
            .arg("--log=error")
            .env("MODULAR_TELEMETRY_ENABLED", "0")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let request = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#;
        let stdin = child.stdin.as_mut().unwrap();
        write!(stdin, "Content-Length: {}\r\n\r\n", request.len()).unwrap();
        stdin.write_all(request).unwrap();
        stdin.flush().unwrap();

        let stdout = child.stdout.take().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || sender.send(read_lsp_message(stdout)).unwrap());
        let response = receiver
            .recv_timeout(std::time::Duration::from_secs(20))
            .expect("Mojo LSP did not answer initialize")
            .unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
        let response: zed::serde_json::Value = zed::serde_json::from_slice(&response).unwrap();
        assert_eq!(response["id"], 1);
        assert!(response["result"]["capabilities"].is_object());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn read_lsp_message(stdout: impl Read) -> std::io::Result<Vec<u8>> {
        let mut reader = BufReader::new(stdout);
        let mut content_length = None;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header)?;
            if header == "\r\n" {
                break;
            }
            if let Some(length) = header.strip_prefix("Content-Length:") {
                content_length = Some(length.trim().parse::<usize>().map_err(|error| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, error)
                })?);
            }
        }
        let mut body = vec![
            0;
            content_length.ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "missing Content-Length")
            })?
        ];
        reader.read_exact(&mut body)?;
        Ok(body)
    }
}
