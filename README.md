# LUpdate

This is a command line tool to generate LU-patcher *compatible* cache directories.

## Usage

Assuming a directory structure like the following, where `{cat1}`, `{cat2}`, …, `{catN}` are directories
with data:

```txt
/src
└──/project
   ├──/main.exe
   ├──/res
   │  ├──{cat1}
   │  ├──{cat2}
   │  ⋮
   │  └──{catN}
   └──/config.txt
/cache
└──/project
```

You need to do the following:

1. Run `lupdate pki /src/project/config.txt`
2. Run `lupdate cache --prefix project --output /cache /src/project`
3. Run `lupdate pack /src /cache`

*Note*: This process may change in the future

## Disclaimer

This tool is intended to facilitate distributing new user-generated content for
private servers. Use is at your own risk. Note that the patcher protocol is using a
public HTTP host, so should you use this tool to prepare a full client, you *are distributing it*
and are liable for any consequences of that.
