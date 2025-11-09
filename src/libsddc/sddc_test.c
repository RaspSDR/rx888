/*
 * sddc_test - simple test program for libsddc
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

#include "libsddc.h"

#if _WIN32
#include <windows.h>
#define sleep(x) Sleep(x * 1000)
#else
#include <unistd.h>
#endif

int main(int argc, char **argv)
{
  /* count devices */
  int count = sddc_get_device_count();
  if (count < 0)
  {
    fprintf(stderr, "ERROR - sddc_get_device_count() failed\n");
    return -1;
  }
  printf("device count=%d\n", count);

  for (int i = 0; i < count; ++i)
  {
    char manufact[256];
    char product[256];
    char serial[256];
    sddc_get_device_usb_strings(i, manufact, product, serial);
    printf("%d - manufacturer=\"%s\" product=\"%s\" serial number=\"%s\"\n",
           i, manufact, product, serial);
  }

  if (count > 0)
  {
    /* open and close device */
    sddc_dev_t *sddc = 0;
    int ret = sddc_open(&sddc, 0);
    if (ret)
    {
      fprintf(stderr, "ERROR - sddc_open() failed\n");
      return -1;
    }
    printf("device opened\n");
    /* done */
    sddc_close(sddc);
  }

  return 0;
}