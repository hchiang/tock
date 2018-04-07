#include "clock.h"

int clock_set(Clock_List_t pin) {
  return command(CLOCK_DRIVER_NUM, 0, pin, 0);
}


