Packaging notes
=================

This directory contains instructions and examples for packaging SDDC components:
- libsddc (runtime shared library + headers)
- soapy (SoapySDR module)
- extio (Windows ExtIO plugin)

Quick local packaging
---------------------

Build the project and run the CPack package target from a build directory:

```bash
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --target package
```

This will produce packages in `build/` according to the CPack configuration in the top-level `CMakeLists.txt`.

Component packages
------------------

The CMake setup uses component-based packaging. To produce only a single component package locally you can run:

```bash
cd build
cpack --config CPackConfig.cmake --component libsddc
cpack --config CPackConfig.cmake --component soapy
cpack --config CPackConfig.cmake --component extio
```

On Linux the CI installs `rpmbuild` so CPack can produce RPMs. Ensure `rpmbuild` and `fakeroot` are available when you want to generate RPMs locally.

Homebrew
--------

Homebrew prefers either building from source or distributing bottles. We've provided a Homebrew formula template in `packaging/homebrew/sddc.rb` — you'll need to update the `url` and `sha256` fields to point at an archive of a tagged release or source tarball. The formula builds with CMake and installs the Soapy module to Homebrew's prefix.

CI notes
--------

The GitHub Actions workflow `/.github/workflows/cmake.yml` has been updated to run CPack on each platform and to normalize artifact names so CI uploads stable artifact filenames per component and platform.

If you want exact, stable artifact filenames adjust either:
- the `CPACK_PACKAGE_FILE_NAME` / generator-specific package file name variables in `CMakeLists.txt`, or
- the CI step that renames generated files (the latter is simpler and has been used in CI already).

Component availability and artifact naming
-----------------------------------------

This project produces component-based packages. Note these platform-specific rules and the CI artifact naming convention:

- Components:
	- `libsddc` — runtime library and headers. Built and packaged on all platforms.
	- `soapy` — SoapySDR module. Built/packaged when SoapySDR is available on the host (Linux/macOS CI installs SoapySDR). Use `cpack --component soapy` from the build directory.
	- `extio` — Windows ExtIO GUI plugin. This is Windows-only and targets x86 (Win32). The ExtIO spec is x86; we do not produce x64 ExtIO packages. Use `cpack --component extio` from the Windows build directory (Win32).

- Artifact naming (CI): all generated package base names use the pattern:

	<component>-<os>-<arch>

	Examples produced by CI:
	- `libsddc-linux-x86_64.tar.gz`
	- `soapy-macos-arm64.zip`
	- `extio-windows-win32.zip`

	The extension (.tar.gz, .zip, etc.) depends on the CPack generator used for that job.

If you want version information in filenames, modify the CI step to set `CPACK_PACKAGE_FILE_NAME` to include `${PROJECT_VERSION}` or set generator-specific naming variables in `CMakeLists.txt`.
