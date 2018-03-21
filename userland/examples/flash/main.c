#include <stdint.h>
#include <stdio.h>

#include <internal/nonvolatile_storage.h>
#include <timer.h>

uint8_t readbuf[256];
uint8_t writebuf[256];

bool done = false;

static void read_done(int length,
                      __attribute__ ((unused)) int arg1,
                      __attribute__ ((unused)) int arg2,
                      __attribute__ ((unused)) void* ud) {
  done = true;
}

static void write_done(int length,
                       __attribute__ ((unused)) int arg1,
                       __attribute__ ((unused)) int arg2,
                       __attribute__ ((unused)) void* ud) {
  done = true;
  //printf("done\n");
}

int main (void) {
  int ret;

  printf("[Nonvolatile Storage] Test App\n");

  ret = nonvolatile_storage_internal_read_buffer(readbuf, 256);
  if (ret != 0) printf("ERROR setting read buffer\n");

  ret = nonvolatile_storage_internal_write_buffer(writebuf, 256);
  if (ret != 0) printf("ERROR setting write buffer\n");

  // Setup callbacks
  ret = nonvolatile_storage_internal_read_done_subscribe(read_done, NULL);
  if (ret != 0) printf("ERROR setting read done callback\n");

  ret = nonvolatile_storage_internal_write_done_subscribe(write_done, NULL);
  if (ret != 0) printf("ERROR setting write done callback\n");


  writebuf[0] = 5;
  writebuf[1] = 10;
  writebuf[2] = 20;
  writebuf[3] = 200;
  writebuf[4] = 123;
  writebuf[5] = 88;
  writebuf[6] = 98;
  writebuf[7] = 89;
  writebuf[8] = 1;
  writebuf[9] = 77;

  while(1) {
    done = false;
    ret  = nonvolatile_storage_internal_write(0, 10);
    if (ret != 0) printf("ERROR calling write\n");
    yield_for(&done);
    delay_ms(1000);
  }

  return 0;
}
