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
