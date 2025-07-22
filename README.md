# LCAP

Lightweight craft authenticator, portable.

## Brief

LCAP is a tiny (~250KiB with UPX) authenticator for code extraction in Microsoft OAuth process.
It provides an affordable way of accomplishing the vanilla login workflow for custom Minecraft launchers.

## Features

- Optimized for size (~300KiB with UPX and ~750KiB unpacked on Windows)
- Easy code retrieval (`stdout` or files / named pipes)
- Configurable partitioning (dedicated cache path for each UUID)
- Works on major platforms (Windows, macOS, GNU/Linux X11 & Wayland)
- Utilizes system webview framework

## Runtime Dependencies

> [!NOTE]
>
> LCAP does not handle these dependencies.
> Instead, they shall be handled by the embedding launcher for better resource management and localization.
> LCAP will likely to panic and return a non-zero exit code upon unsatisfied dependencies.

### Windows

Requires [Edge WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2).
Should be bundled with up-to-date Windows versions, yet can also be installed separately.

### macOS

Uses [WKWebView](https://developer.apple.com/documentation/webkit/wkwebview), thus no additional setup needed.

### GNU/Linux

Requires [WebKitGTK](https://webkitgtk.org), can be installed from the system package manager.

## Usage

### Quick Start

For a simple test or demo projects, just run the binary:

```shell
./LCAP
```

*Examples are written for POSIX shells, yet Windows version should be very similar.*

Continue the login inside the popped up window and the authorization code shall be printed to `stdout`:

```shell
LCAP:CODE=YOUR_CODE_HERE
```

If the login process failed due to remote reasons, the error message will also be printed:

```shell
LCAP:ERR=ERROR_MSG_HERE
```

### Specify a UUID

LCAP generates a random UUID for storing the browser cache data (cookies, storage, etc.).
This creates a "fresh" login environment each time.
To utilize the fact that Microsoft accounts can stay logged-in, specify a UUID manually:

```shell 
./LCAP -p 019739cc-0d8e-78f0-98ee-57737e013271  # A random generated one
```

As long as the same UUID is provided, the created instances will share the same copy of locally stored data, resulting
an auto-login if applicable.

### Customize the Title

The default title "LCAP" can be confusing. This can be customized:

```shell
./LCAP -t YOUR_TITLE_HERE
```

### Use File Output

Grabbing the `stdout` is an easy approach for simple use cases.
For a safer and more robust way of retrieving the result, use a named pipe:

```shell
mkfifo PIPE_NAME_HERE
./LCAP -f PIPE_NAME_HERE
cat PIPE_NAME_HERE  # LCAP:CODE=YOUR_CODE_HERE
```

A file can also be used in case when named pipes are not supported.

### Wait Timeout

By default, LCAP tries to make auto-login seamless and does not show the window until it knows for sure that user
interaction is needed, or a specified timeout has elapsed (in case the page failed to load). The default value is 5
seconds, and can be customized:

```shell
./LCAP -w 1000  # Shows the window once the login page loads, or after 1 seconds
```

## Build

> [!NOTE]
>
> To minimize the binary size, we've toggled a set of compile options which may affect build performance.
> Check `Cargo.toml` if adjustments are needed.

LCAP uses [saucers](https://github.com/skjsjhb/saucers), so dependencies listed there are also required to build LCAP.
Make sure to check it out!

Clone the project:

```shell
git clone https://github.com/skjsjhb/LCAP.git --filter=tree:0
```

This project has configured the corresponding toolchain to use, so simply do:

```shell
cargo build --release
```

On Windows, due to limitations of LTO, the target may need to be specified explicitly:

```shell
cargo build --release --target x86_64-pc-windows-msvc # or aarch64-pc-windows-msvc
```

## License

LCAP is licensed under the [GNU General Public License](https://www.gnu.org/licenses/gpl-3.0.html), version 3 or later.
