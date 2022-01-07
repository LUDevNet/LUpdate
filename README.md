# LUpdate

This is a command line tool to generate LU-patcher *compatible* cache directories.

## Usage

Assuming a directory structure like the following, where `{cat1}`, `{cat2}`, …, `{catN}` are directories
with data:

```txt
/LUpdate.toml
/dev
├──/project
│  ├──/MyProject.exe
│  ├──/res
│  │  ├──{cat1}
│  │  ├──{cat2}
│  │  ⋮
│  │  └──{catN}
│  └──/config.txt
└──/server
   ├──/MyServer.exe
   ├──/res
   │  ├──{cat1}
   │  ├──{cat2}
   │  ⋮
   │  └──{catN}
   └──/config.txt
/cache
├──/luserver
└──/luproject
```

You need to do the following:

1. Run `lupdate pki` to generate `primary.pki`
2. Run `lupdate cache` to populate the sd0 cache and create `trunk.txt`
3. Run `lupdate pack` to pre-package all PK-archives with `front` (`--filter *front*`)
4. Run `lupdate cache` again to cache PK files
5. Cut down `trunk.txt` to what the frontend needs

*Note*: This process may change in the future

## Sample config file

Save the following file as `LUpdate.toml` in the root directory (i.e. next to `dev`).
For the PKI you need another config file in the project dir (i.e. `server` / `project`)

```toml
[general]
src = "dev"

[project.luserver]
dir = "server"
config = "config.txt"
cache = "cache"
```

## PKI Config

Example:

```
pack=pack\cat1.pk
add_dir=cat1\sub1
add_dir=cat1\sub3
end_pack

pack=pack\cat2.pk
add_dir=cat2\subA
add_dir=cat2\subX
end_pack
```

## Disclaimer

This tool is intended to facilitate distributing new user-generated content for
private servers. Use is at your own risk. Note that the patcher protocol is using a
public HTTP host, so should you use this tool to prepare a full client, you *are distributing it*
and are liable for any consequences of that.
