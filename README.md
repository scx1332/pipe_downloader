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

It spawns 3 or more threads per download, one or more for downloading, 
one for decompressing and one for untaring (writing files also).
It is using chunks, when chunks are filled with data they are gathered and
when next chunk is full they are processed by unpacker.

No unsafe code used.

Currently, it supports:
* archive.tar.lz4
* archive.tar.gz
* archive.tar.bz2
* archive.tar.zst
* archive.tar.xz
* single_file.lz4
* single_file.gz
* single_file.bz2
* single_file.zst
* single_file.xz

As base mode of operation it used PARTIAL_CONTENT (206) http status code, 
which allows to continue operation when connection is lost (downloader waits patiently until chunk is available again).
Downloader also supports normal GET (200) download when server does not support PARTIAL_CONTENT, but any network timeout/disconnection will lead 
to unrecoverable error then.

There is no option of restarting download after unrecoverable error or killed process.

1. Cross compilation

```cross build --release --target aarch64-unknown-linux-musl```

2. Test runs

```cargo run --release --example pipe_downloader -- --output-dir linux-6.0.4 --url=https://mirrors.edge.kernel.org/pub/linux/kernel/v6.x/linux-6.0.4.tar.gz```
