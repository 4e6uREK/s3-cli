# 4e6uREK S3 CLI Implementation

## Features
1. Get file
2. Put file
3. Dump bucket
4. Restore bucket
5. List bucket files

## How to use

First of you must provide configuration for this software. Configuration is made in Old School INI format.
You can provide INI configuration using -c, --config options or write it in $HOME/.config/s3-cli/config.ini

Example INI config:
```
domain=http://localhost:9000
region=my-region
access_key=<ACCESS_KEY>
secret_key=<SECRET_kEY>
bucket=test
```

1. Get file
```sh
true-s3-cli -r <filename>
```

2. Put file
```sh
true-s3-cli -s <filename>
```

3. Dump bucket
```sh
true-s3-cli -d
```

4. Restore bucket
```
true-s3-cli -p <archive.tar>
```

5. List files
```sh
true-s3-cli -l
```
