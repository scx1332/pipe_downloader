# pipe_downloader

This program is used to download, uncompress and untar files at once.
So basically it does the same as command:

```
curl http://arvhive.tar.lz4 | lz4 -dc | tar -xv -C output_dir
```
result is the same as using (pipe_downloader will create directory if not empty):
```
pipe_downloader --url http://arvhive.tar.lz4 --output-dir output_dir
```

but it uses multiple cores better than piped commands like above.
Additionally, it provides progress, that can be used by server to provide API.
Implementation is nonblocking, based on threads (no async IO used)

It spawns 3 threads per download, one for downloading, 
one for decompressing and one for untaring (writing files also). Simple mspc is used to cross thread data share 
(no ring buffers or similar solution). Some unnecessary memory copying is still done, but it doesn't affect performance a lot.

No unsafe code used for now.

Currently, it supports:
* archive.tar.lz4
* archive.tar.gz
* archive.tar.bz2
* single_file.lz4
* single_file.gz
* single_file.bz2

As base mode of operation it used PARTIAL_CONTENT (206) http status code, 
which allows to continue operation when connection is lost (downloader waits patiently until chunk is available again).
Downloader also supports normal GET (200) download when server does not support PARTIAL_CONTENT, but any network timeout/disconnection will lead 
to unrecoverable error then.

Right now there is no option of restarting download after unrecoverable error or killed process.

1. Cross compilation

```cross build --release --target aarch64-unknown-linux-musl```

