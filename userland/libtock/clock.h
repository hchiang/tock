#pragma once

#include "tock.h"

#ifdef __cplusplus
extern "C" {
#endif

#define CLOCK_DRIVER_NUM 0x1C

typedef enum {
  DEFAULT,
  RC1M,
  RCFAST4M,
  RCFAST8M,
  RCFAST12M,
  EXTOSC,
  DFLL,
  PLL,
  RC80M,
  RCSYS,
} Clock_List_t;

int clock_set(Clock_List_t clock);

#ifdef __cplusplus
}
#endif

