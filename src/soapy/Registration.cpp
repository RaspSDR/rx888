#include <cstdint>
#include <map>

#include "SoapySDDC.hpp"
#include <SoapySDR/Registry.hpp>

SoapySDR::KwargsList findSDDC(const SoapySDR::Kwargs &args)
{
    std::vector<SoapySDR::Kwargs> results;

    const size_t this_count = sddc_get_device_count();

    for (size_t i = 0; i < this_count; i++)
    {
        char manufact[256], product[256], serial[256];

        if (sddc_get_device_usb_strings(i, manufact, product, serial) != 0)
        {
            continue;
        }

        //filtering by serial
        if (args.count("serial") != 0 and args.at("serial") != serial) continue;

        SoapySDR::Kwargs devInfo;
        devInfo["label"] = std::string(sddc_get_device_name(i)) + " :: " + serial;
        devInfo["product"] = product;
        devInfo["serial"] = serial;
        devInfo["manufacturer"] = manufact;

        results.push_back(devInfo);
    }

    return results;
}

SoapySDR::Device *makeSDDC(const SoapySDR::Kwargs &args)
{
    return new SoapySDDC(args);
}

static SoapySDR::Registry registerSDDC("SDDC", &findSDDC, &makeSDDC, SOAPY_SDR_ABI_VERSION);
