#include <cstdint>
#include <sys/types.h>
#include <cstring>

#include "SoapySDDC.hpp"
#include <SoapySDR/Types.hpp>
#include <SoapySDR/Time.hpp>

static void _Callback(void *context, const float *data, uint32_t len)
{
    SoapySDDC *sddc = (SoapySDDC *)context;
    sddc->Callback(context, data, len);
}

int SoapySDDC::Callback(void *context, const float *data, uint32_t len)
{
    // DbgPrintf("SoapySDDC::Callback %d\n", len);
    if (_buf_count == numBuffers)
    {
        _overflowEvent = true;
        return 0;
    }

    auto &buff = _buffs[_buf_tail];
    buff.resize(len * bytesPerSample);
    memcpy(buff.data(), data, len * bytesPerSample);
    _buf_tail = (_buf_tail + 1) % numBuffers;

    {
        std::lock_guard<std::mutex> lock(_buf_mutex);
        _buf_count++;
    }
    _buf_cond.notify_one();

    return 0;
}

SoapySDDC::SoapySDDC(const SoapySDR::Kwargs &args) : deviceId(-1),
                                                     numBuffers(16),
                                                     sampleRate(32000000)
{
    //if a serial is not present, then findRTLSDR had zero devices enumerated
    if (args.count("serial") == 0) throw std::runtime_error("No RTL-SDR devices found!");

    const auto serial = args.at("serial");
    deviceId = sddc_get_index_by_serial(serial.c_str());
    if (deviceId < 0) throw std::runtime_error("rtlsdr_get_index_by_serial("+serial+") - " + std::to_string(deviceId));

    sddc_open(&this->sddc, deviceId);
}

SoapySDDC::~SoapySDDC(void)
{
    DbgPrintf("SoapySDDC::~SoapySDDC\n");
    sddc_close(this->sddc);
}

std::string SoapySDDC::getDriverKey(void) const
{
    DbgPrintf("SoapySDDC::getDriverKey\n");
    return "SDDC";
}

std::string SoapySDDC::getHardwareKey(void) const
{
    DbgPrintf("SoapySDDC::getHardwareKey\n");

    return std::string(sddc_get_device_name(this->deviceId));
}

SoapySDR::Kwargs SoapySDDC::getHardwareInfo(void) const
{
    // key/value pairs for any useful information
    // this also gets printed in --probe
    SoapySDR::Kwargs args;

    args["origin"] = "https://github.com/ik1xpv/ExtIO_sddc";
    args["index"] = std::to_string(deviceId);

    DbgPrintf("SoapySDDC::getHardwareInfo\n");
    return args;
}

/*******************************************************************
 * Channels API
 ******************************************************************/

size_t SoapySDDC::getNumChannels(const int dir) const
{
    DbgPrintf("SoapySDDC::getNumChannels\n");
    return (dir == SOAPY_SDR_RX) ? 1 : 0;
}

bool SoapySDDC::getFullDuplex(const int, const size_t) const
{
    DbgPrintf("SoapySDDC::getFullDuplex\n");
    return false;
}

/*******************************************************************
 * Antenna API
 ******************************************************************/

std::vector<std::string> SoapySDDC::listAntennas(const int direction, const size_t) const
{
    DbgPrintf("SoapySDDC::listAntennas : %d\n", direction);
    std::vector<std::string> antennas;
    if (direction == SOAPY_SDR_TX)
    {
        return antennas;
    }

    antennas.push_back("HF");
    antennas.push_back("VHF");
    // i want to list antennas names in dbgprintf
    for (auto &antenna : antennas)
    {
        DbgPrintf("SoapySDDC::listAntennas : %s\n", antenna.c_str());
    }
    return antennas;
}

// set the selected antenna
void SoapySDDC::setAntenna(const int direction, const size_t, const std::string &name)
{
    DbgPrintf("SoapySDDC::setAntenna : %d\n", direction);
    if (direction != SOAPY_SDR_RX)
    {
        return;
    }
    if (name == "HF")
    {
        sddc_set_direct_sampling(this->sddc, true);
    }
    else if (name == "VHF")
    {
        sddc_set_direct_sampling(this->sddc, false);
    }

    // what antenna is set print in dbgprintf
    DbgPrintf("SoapySDDC::setAntenna : %s\n", name.c_str());
}

// get the selected antenna
std::string SoapySDDC::getAntenna(const int direction, const size_t) const
{
    DbgPrintf("SoapySDDC::getAntenna\n");

    if (sddc_get_direct_sampling(this->sddc))
    {
        return "HF";
    }
    else
    {
        return "VHF";
    }
}

bool SoapySDDC::hasDCOffsetMode(const int, const size_t) const
{
    DbgPrintf("SoapySDDC::hasDCOffsetMode\n");
    return false;
}

bool SoapySDDC::hasFrequencyCorrection(const int, const size_t) const
{
    DbgPrintf("SoapySDDC::hasFrequencyCorrection\n");
    return false;
}

void SoapySDDC::setFrequencyCorrection(const int, const size_t, const double)
{
    DbgPrintf("SoapySDDC::setFrequencyCorrection\n");
}

double SoapySDDC::getFrequencyCorrection(const int, const size_t) const
{
    DbgPrintf("SoapySDDC::getFrequencyCorrection\n");
    return 0.0;
}

std::vector<std::string> SoapySDDC::listGains(const int, const size_t) const
{
    DbgPrintf("SoapySDDC::listGains\n");
    std::vector<std::string> gains;
    gains.push_back("RF");
    gains.push_back("IF");
    return gains;
}

bool SoapySDDC::hasGainMode(const int, const size_t) const
{
    DbgPrintf("SoapySDDC::hasGainMode\n");
    return false;
}

// void SoapySDDC::setGainMode(const int, const size_t, const bool)

// bool SoapySDDC::getGainMode(const int, const size_t) const

// void SoapySDDC::setGain(const int, const size_t, const double)

void SoapySDDC::setGain(const int, const size_t, const std::string &name, const double value)
{
    DbgPrintf("SoapySDDC::setGain %s = %f\n", name.c_str(), value);

    if (name == "RF") {
        sddc_set_rf_attenuator(this->sddc, (int)value);
    }
    else if (name == "IF") {
        sddc_set_if_gain(this->sddc, (int)value);
    }
}

SoapySDR::Range SoapySDDC::getGainRange(const int direction, const size_t channel, const std::string &name) const
{
    DbgPrintf("SoapySDDC::getGainRange %s\n", name.c_str());
    float min, max;

    if (name == "RF") {
        sddc_get_rf_attenuator_range(this->sddc, &min, &max);
        return SoapySDR::Range(
            min, max
        );
    }
    else if (name == "IF") {
        sddc_get_if_gain_range(this->sddc, &min, &max);
        return SoapySDR::Range(
            min,
            max
        );
    }
    else
        return SoapySDR::Range();
}

void SoapySDDC::setFrequency(const int, const size_t, const double frequency, const SoapySDR::Kwargs &)
{
    DbgPrintf("SoapySDDC::setFrequency %f\n", frequency);
    sddc_set_center_freq64(this->sddc, (uint64_t)frequency);
}

void SoapySDDC::setFrequency(const int, const size_t, const std::string &, const double frequency, const SoapySDR::Kwargs &)
{
    DbgPrintf("SoapySDDC::setFrequency\n");
    sddc_set_center_freq64(this->sddc, (uint64_t)frequency);
}

double SoapySDDC::getFrequency(const int, const size_t) const
{
    DbgPrintf("SoapySDDC::getFrequency %f\n", (double)centerFrequency);
    return (double)sddc_get_center_freq64(this->sddc);
}

double SoapySDDC::getFrequency(const int, const size_t, const std::string &name) const
{
    return (double)sddc_get_center_freq64(this->sddc);
}

std::vector<std::string> SoapySDDC::listFrequencies(const int direction, const size_t channel) const
{
    std::vector<std::string> ret;

    if (channel == 0) {
        ret.push_back("RF");
    }

    return ret;
}

SoapySDR::RangeList SoapySDDC::getFrequencyRange(const int direction, const size_t channel, const std::string &name) const
{
    SoapySDR::RangeList ranges;

    ranges.push_back(SoapySDR::Range(10000, 1800000000));

    return ranges;
}

SoapySDR::ArgInfoList SoapySDDC::getFrequencyArgsInfo(const int, const size_t) const
{
    DbgPrintf("SoapySDDC::getFrequencyArgsInfo\n");
    return SoapySDR::ArgInfoList();
}

void SoapySDDC::setSampleRate(const int, const size_t, const double rate)
{
    DbgPrintf("SoapySDDC::setSampleRate %f\n", rate);
    switch ((int)rate)
    {
    case 32000000:
        sampleRate = 32000000;
        samplerateidx = 4;
        break;
    case 16000000:
        sampleRate = 16000000;
        samplerateidx = 3;
        break;
    case 8000000:
        sampleRate = 8000000;
        samplerateidx = 2;
        break;
    case 4000000:
        sampleRate = 4000000;
        samplerateidx = 1;
        break;
    case 2000000:
        sampleRate = 2000000;
        samplerateidx = 0;
        break;
    default:
        return;
    }
}

double SoapySDDC::getSampleRate(const int, const size_t) const
{
    DbgPrintf("SoapySDDC::getSampleRate %f\n", sampleRate);
    return sampleRate;
}

std::vector<double> SoapySDDC::listSampleRates(const int, const size_t) const
{
    DbgPrintf("SoapySDDC::listSampleRates\n");
    std::vector<double> results;

    results.push_back(2000000);
    results.push_back(4000000);
    results.push_back(8000000);
    results.push_back(16000000);
    results.push_back(32000000);

    return results;
}

SoapySDR::ArgInfoList SoapySDDC::getSettingInfo(void) const
{
    SoapySDR::ArgInfoList setArgs;

    SoapySDR::ArgInfo BiasTHFArg;
    BiasTHFArg.key = "UpdBiasT_HF";
    BiasTHFArg.value = "false";
    BiasTHFArg.name = "HF Bias Tee enable";
    BiasTHFArg.description = "Enabe Bias Tee on HF antenna port";
    BiasTHFArg.type = SoapySDR::ArgInfo::BOOL;
    setArgs.push_back(BiasTHFArg);

    SoapySDR::ArgInfo BiasTVHFArg;
    BiasTVHFArg.key = "UpdBiasT_VHF";
    BiasTVHFArg.value = "false";
    BiasTVHFArg.name = "VHF Bias Tee enable";
    BiasTVHFArg.description = "Enabe Bias Tee on VHF antenna port";
    BiasTVHFArg.type = SoapySDR::ArgInfo::BOOL;
    setArgs.push_back(BiasTVHFArg);

    return setArgs;
}

void SoapySDDC::writeSetting(const std::string &key, const std::string &value)
{
    int biasTee;
    if (key == "UpdBiasT_HF")
    {
        biasTee = (value == "true") ? 0x01: 0;
    }
    else if (key == "UpdBiasT_VHF")
    {
        biasTee = (value == "true") ? 0x02: 0;
    }

    sddc_set_bias_tee(this->sddc, biasTee);
}


// void SoapySDDC::setMasterClockRate(const double rate)
// {
//     DbgPrintf("SoapySDDC::setMasterClockRate %f\n", rate);
//     masterClockRate = rate;
// }

// double SoapySDDC::getMasterClockRate(void) const
// {
//     DbgPrintf("SoapySDDC::getMasterClockRate %f\n", masterClockRate);
//     return masterClockRate;
// }

// std::vector<std::string> SoapySDDC::listTimeSources(void) const
// {
//     DbgPrintf("SoapySDDC::listTimeSources\n");
//     std::vector<std::string> sources;
//     sources.push_back("sw_ticks");
//     return sources;
// }

// std::string SoapySDDC::getTimeSource(void) const
// {
//     DbgPrintf("SoapySDDC::getTimeSource\n");
//     return "sw_ticks";
// }

// bool SoapySDDC::hasHardwareTime(const std::string &what) const
// {
//     DbgPrintf("SoapySDDC::hasHardwareTime\n");
//     return what == "" || what == "sw_ticks";
// }

// long long SoapySDDC::getHardwareTime(const std::string &what) const
// {
//     DbgPrintf("SoapySDDC::getHardwareTime\n");
//     return SoapySDR::ticksToTimeNs(ticks, sampleRate);
// }

// void SoapySDDC::setHardwareTime(const long long timeNs, const std::string &what)
// {
//     DbgPrintf("SoapySDDC::setHardwareTime\n");
//     ticks = SoapySDR::timeNsToTicks(timeNs, sampleRate);
// }
