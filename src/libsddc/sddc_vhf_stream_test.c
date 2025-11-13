/*
 * sddc_vhf_stream_test - simple VHF/UHF stream test program for libsddc
 *
 * Copyright (C) 2020 by Franco Venturi
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "sddc.h"
#include "wavewrite.h"

#if _WIN32
#include "win_clock.h"
#endif

static unsigned long long received_samples = 0;
static unsigned long long total_samples = 0;
static int num_callbacks;
static int16_t *sampleData = 0;
static int runtime = 3000;
static struct timeval clk_start, clk_end;
static int stop_reception = 0;

static inline double clk_diff()
{
  return ((double)clk_end.tv_sec + 1.0e-6 * clk_end.tv_usec) -
         ((double)clk_start.tv_sec + 1.0e-6 * clk_start.tv_usec);
}


static void count_bytes_callback(unsigned char *buf, uint32_t len, void *ctx)
{
  if (stop_reception)
    return;
  ++num_callbacks;
  unsigned N = len / sizeof(int16_t);
  if (received_samples + N < total_samples)
  {
    if (sampleData)
      memcpy(sampleData + received_samples, buf, len);
    received_samples += N;
  }
  else
  {
    clock_gettime(CLOCK_REALTIME, &clk_end);
    stop_reception = 1;
    sddc_cancel_async((sddc_dev_t *)ctx);
  }
}

int main(int argc, char **argv)
{
  if (argc < 3)
  {
    fprintf(stderr, "usage: %s <sample rate> <vhf frequency> [<runtime_in_ms> [<output_filename>]\n", argv[0]);
    return -1;
  }
  const char *outfilename = NULL;
  uint32_t sample_rate = 0.0;
  uint64_t vhf_frequency = 100 * 1000 * 1000; /* 100 MHz */
  int vhf_attenuation = -20;                  /* 20dB attenuation */

  sscanf(argv[1], "%ld", &sample_rate);
  sscanf(argv[2], "%lld", &vhf_frequency);
  if (3 < argc)
    runtime = atoi(argv[3]);
  if (4 < argc)
    outfilename = argv[4];

  if (sample_rate <= 0)
  {
    fprintf(stderr, "ERROR - given samplerate '%f' should be > 0\n", sample_rate);
    return -1;
  }

  int ret_val = -1;

  sddc_dev_t *sddc;
  int ret = sddc_open_raw(&sddc, 0);
  if (ret < 0)
  {
    fprintf(stderr, "ERROR - sddc_open() failed\n");
    return -1;
  }

  if (sddc_set_sample_rate(sddc, sample_rate) < 0)
  {
    fprintf(stderr, "ERROR - sddc_set_sample_rate() failed\n");
    goto DONE;
  }

  // enable tuner
  if (sddc_set_direct_sampling(sddc, 0) < 0)
  {
    fprintf(stderr, "ERROR - sddc_set_direct_sampling() failed\n");
    goto DONE;
  }

  if (sddc_set_center_freq64(sddc, vhf_frequency) < 0)
  {
    fprintf(stderr, "ERROR - sddc_set_vhf_frequency() failed\n");
    goto DONE;
  }

  if (sddc_set_rf_attenuator(sddc, vhf_attenuation) < 0)
  {
    fprintf(stderr, "ERROR - sddc_set_rf_attenuator() failed\n");
    goto DONE;
  }

  received_samples = 0;
  num_callbacks = 0;

  fprintf(stderr, "started streaming .. for %d ms ..\n", runtime);
  total_samples = ((unsigned long long)runtime * sample_rate / 1000.0);

  if (outfilename)
    sampleData = (int16_t *)malloc(total_samples * sizeof(int16_t));

  /* todo: move this into a thread */
  stop_reception = 0;
  clock_gettime(CLOCK_REALTIME, &clk_start);

  sddc_read_async(sddc, count_bytes_callback, sddc, 0, 0);

  fprintf(stderr, "finished. now stop streaming ..\n");

  double dur = clk_diff();
  fprintf(stderr, "received=%llu 16-Bit samples in %d callbacks\n", received_samples, num_callbacks);
  fprintf(stderr, "run for %f sec\n", dur);
  fprintf(stderr, "approx. samplerate is %f kSamples/sec\n", received_samples / (1000.0 * dur));

  if (outfilename && sampleData && received_samples)
  {
    FILE *f = fopen(outfilename, "wb");
    if (f)
    {
      fprintf(stderr, "saving received real samples to file ..\n");
      waveWriteHeader((unsigned)(0.5 + sample_rate), 0U /*frequency*/, 16 /*bitsPerSample*/, 1 /*numChannels*/, f);
      for (unsigned long long off = 0; off + 65536 < received_samples; off += 65536)
        waveWriteSamples(f, sampleData + off, 65536, 0 /*needCleanData*/);
      waveFinalizeHeader(f);
      fclose(f);
    }
  }

  /* done - all good */
  ret_val = 0;

DONE:
  sddc_close(sddc);

  return ret_val;
}
