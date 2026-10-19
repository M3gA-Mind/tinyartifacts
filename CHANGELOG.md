# Changelog

## 0.2.0

- Add the `FileStore` port with filesystem and blob-store adapters so file
  versions can be shared across host processes.
- Store `FileRef` values as opaque keys with size and media type, and preserve
  deserialization of legacy path-based references for import.
- Configure file storage with `Artifacts::with_file_store` or
  `Artifacts::with_version_files` (filesystem compatibility helper).
- This changes the public `FileRef` fields and file-storage setup API. Hosts
  must provide a file store for file artifacts.
