#include "sddc.h"

#include "CppUnitTestFramework.hpp"
#include <thread>
#include <chrono>

using namespace std::chrono;

namespace
{
    struct SddcFixture
    {
        sddc_dev_t *sddc;

        int stop_reception = 0;
        unsigned long long received_samples = 0;
        unsigned long long total_samples = 0;
        int num_callbacks = 0;
    };
}

template <typename T>
void count_samples_callback(unsigned char *buf, uint32_t len, void *ctx)
{
    struct SddcFixture *fixture = (struct SddcFixture *)ctx;

    if (fixture->stop_reception)
        return;
    ++fixture->num_callbacks;
    unsigned N = len / sizeof(T);
    fixture->received_samples += N;

    if (fixture->received_samples >= fixture->total_samples)
    {
        fixture->stop_reception = 1;
        sddc_cancel_async(fixture->sddc);
    }
}

TEST_CASE(SddcFixture, UsbEnumeration)
{
    int count = sddc_get_device_count();

    REQUIRE_EQUAL(count, 1);

    char manufact[256];
    char product[256];
    char serial[256];
    int ret = sddc_get_device_usb_strings(0, manufact, product, serial);
    REQUIRE_EQUAL(ret, 0);

    int index = sddc_get_index_by_serial(serial);
    REQUIRE_EQUAL(index, 0);

    const char *name = sddc_get_device_name(0);
    REQUIRE_EQUAL(strcmp(name, product), 0);
}

TEST_CASE(SddcFixture, RawDirectSamplingMode)
{
    int ret;
    uint32_t sample_rate = 32000000;

    ret = sddc_open_raw(&sddc, 0);
    REQUIRE_EQUAL(ret, 0);

    ret = sddc_set_direct_sampling(sddc, 1);
    REQUIRE_EQUAL(ret, 0);

    ret = sddc_set_sample_rate(sddc, sample_rate);
    REQUIRE_EQUAL(ret, 0);

    int ds = sddc_get_direct_sampling(sddc);
    REQUIRE_EQUAL(ds, 1);

    received_samples = 0;
    num_callbacks = 0;

    total_samples = ((unsigned long long)1000 * sample_rate / 1000.0);
    stop_reception = 0;

    const auto start{steady_clock::now()};
    ret = sddc_read_async(sddc, count_samples_callback<int16_t>, dynamic_cast<struct SddcFixture*>(this), 0, 0);
    REQUIRE_EQUAL(ret, 0);
    const auto finish{steady_clock::now()};

    const duration<double> elapsed_seconds{finish - start};
    std::cout << "received=" << received_samples << " 16-Bit samples in " << num_callbacks << " callbacks\n";
    std::cout << "Elapsed time: " << elapsed_seconds.count() << "s\n";
    std::cout << "approx. samplerate is " << received_samples / (1000.0 * elapsed_seconds.count()) << " kSamples/sec\n";

    ret = sddc_close(sddc);
    REQUIRE_EQUAL(ret, 0);
}


TEST_CASE(SddcFixture, RawTunerMode)
{
    int ret;
    uint32_t sample_rate = 32000000;

    ret = sddc_open_raw(&sddc, 0);
    REQUIRE_EQUAL(ret, 0);

    ret = sddc_set_direct_sampling(sddc, 0);
    REQUIRE_EQUAL(ret, 0);

    ret = sddc_set_sample_rate(sddc, sample_rate);
    REQUIRE_EQUAL(ret, 0);

    int ds = sddc_get_direct_sampling(sddc);
    REQUIRE_EQUAL(ds, 0);

    received_samples = 0;
    num_callbacks = 0;

    total_samples = ((unsigned long long)1000 * sample_rate / 1000.0);
    stop_reception = 0;

    const auto start{steady_clock::now()};
    ret = sddc_read_async(sddc, count_samples_callback<int16_t>, dynamic_cast<struct SddcFixture*>(this), 0, 0);
    REQUIRE_EQUAL(ret, 0);
    const auto finish{steady_clock::now()};

    const duration<double> elapsed_seconds{finish - start};
    std::cout << "received=" << received_samples << " 16-Bit samples in " << num_callbacks << " callbacks\n";
    std::cout << "Elapsed time: " << elapsed_seconds.count() << "s\n";
    std::cout << "approx. samplerate is " << received_samples / (1000.0 * elapsed_seconds.count()) << " kSamples/sec\n";

    ret = sddc_close(sddc);
    REQUIRE_EQUAL(ret, 0);
}

TEST_CASE(SddcFixture, HFSamplingMode)
{
    int ret;
    uint32_t sample_rate = 8000000;

    ret = sddc_open(&sddc, 0);
    REQUIRE_EQUAL(ret, 0);

    ret = sddc_set_direct_sampling(sddc, 1);
    REQUIRE_EQUAL(ret, 0);

    ret = sddc_set_sample_rate(sddc, sample_rate);
    REQUIRE_EQUAL(ret, 0);

    int ds = sddc_get_direct_sampling(sddc);
    REQUIRE_EQUAL(ds, 1);

    received_samples = 0;
    num_callbacks = 0;

    total_samples = ((unsigned long long)1000 * sample_rate / 1000.0);
    stop_reception = 0;
    
    const auto start{steady_clock::now()};
    ret = sddc_read_async(sddc, count_samples_callback<float>, dynamic_cast<struct SddcFixture*>(this), 0, 0);
    REQUIRE_EQUAL(ret, 0);
    const auto finish{steady_clock::now()};

    const duration<double> elapsed_seconds{finish - start};

    std::cout << "received=" << received_samples << " 16-Bit samples in " << num_callbacks << " callbacks\n";
    std::cout << "Elapsed time: " << elapsed_seconds.count() << "s\n";
    std::cout << "approx. samplerate is " << received_samples / (1000.0 * elapsed_seconds.count()) << " kSamples/sec\n";

    ret = sddc_close(sddc);
    REQUIRE_EQUAL(ret, 0);
}