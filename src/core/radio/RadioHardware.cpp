#include "RadioHardware.h"

bool RadioHardware::FX3SetGPIO(uint32_t mask)
{
    gpios |= mask;

    return Fx3->Control(GPIOFX3, gpios);
}

bool RadioHardware::FX3UnsetGPIO(uint32_t mask)
{
    gpios &= ~mask;

    return Fx3->Control(GPIOFX3, gpios);
}

RadioHardware::~RadioHardware()
{
    if (Fx3) {
        FX3SetGPIO(SHDWN);
    }
}

extern RadioHardware* CreateRadioHardwareInstance(fx3class* fx3)
{
    RadioHardware *hardware = nullptr;
    uint8_t rdata[4];
    fx3->GetHardwareInfo((uint32_t*)rdata);
	RadioModel radio = (RadioModel)rdata[0];
	uint16_t firmware = (rdata[1] << 8) + rdata[2];

    switch (radio)
	{
	case HF103:
		hardware = new HF103Radio(fx3);
		break;

	case BBRF103:
		hardware = new BBRF103Radio(fx3);
		break;

	case RX888:
		hardware = new RX888Radio(fx3);
		break;

	case RX888r2:
		hardware = new RX888R2Radio(fx3);
		break;

	case RX888r3:
		hardware = new RX888R3Radio(fx3);
		break;

	case RX999:
		hardware = new RX999Radio(fx3);
		break;

	case RXLUCY:
		hardware = new RXLucyRadio(fx3);
		break;

	default:
		hardware = new DummyRadio(fx3);
		DbgPrintf("WARNING no SDR connected\n");
		break;
	}

    DbgPrintf("%s | firmware %x\n", hardware->getName(), firmware);

    return hardware;
}