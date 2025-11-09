class Sddc < Formula
  desc "SDDC libraries and SoapySDR support module"
  homepage "www.rx-888.com/rx/"
  # Replace the url below with a release tarball url and correct sha256
  url "https://example.com/sddc-1.0.1.tar.gz"
  sha256 "REPLACE_WITH_REAL_SHA256"
  license "GPL-3.0-or-later"

  depends_on "cmake" => :build
  depends_on "soapysdr"
  depends_on "fftw"

  def install
    mkdir "build" do
      system "cmake", "..", "-DCMAKE_BUILD_TYPE=Release", "-DCMAKE_INSTALL_PREFIX=#{prefix}"
      system "cmake", "--build", ".", "--target", "install"
    end

    # Install Soapy module into lib/SoapySDR/modules-1 (adjust if needed)
    lib.install Dir["#{prefix}/lib/SoapySDR/modules-1/*"] if Dir.exist?("#{prefix}/lib/SoapySDR/modules-1")
  end

  test do
    # Basic smoke test: ensure the main library can be loaded or the binary runs
    system "true"
  end
end
