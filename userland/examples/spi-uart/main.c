#include <stdbool.h>
#include <gpio.h>
#include <spi.h>

#define BUF_SIZE 200
char rbuf[BUF_SIZE];
char wbuf[BUF_SIZE];
char receive_buf[10];
bool receive = false;
bool interrupt = false;

static void write_cb(__attribute__ ((unused)) int arg0,
                     __attribute__ ((unused)) int arg2,
                     __attribute__ ((unused)) int arg3,
                     __attribute__ ((unused)) void* userdata) {
}

static void receive_cb(__attribute__ ((unused)) int arg0,
                     __attribute__ ((unused)) int arg2,
                     __attribute__ ((unused)) int arg3,
                     __attribute__ ((unused)) void* userdata) {
    receive = true;
}

static void gpio_cb(__attribute__ ((unused)) int arg0,
                     __attribute__ ((unused)) int arg2,
                     __attribute__ ((unused)) int arg3,
                     __attribute__ ((unused)) void* userdata) {
    interrupt = true;
}


int main(void) {
    int i;
    for (i = 0; i < 200; i++) {
      wbuf[i] = i;
    }
    spi_set_chip_select(0);
    spi_set_rate(400000);
    spi_set_polarity(false);
    spi_set_phase(false);

/*
    //setup interrupt callback 
    gpio_interrupt_callback(gpio_cb,NULL);
    gpio_enable_input(0,PullDown);
    gpio_enable_interrupt(0,Change);

    //TODO Add something to libtock library to allow receiving from RX/TX pins
    //setup receive buffer
    serialization_subscribe(receive_cb);
    serialization_setup_rx_buffer(receive_buf,10);

    while(1) {
        // Wait for an interrupt before you start to receive
        yield_for(&interrupt);
        interrupt = false;

        yield_for(&receive);
        receive = false;

        if strcmp(receive_buf,"Start") {
*/
            spi_read_write(wbuf, rbuf, BUF_SIZE, write_cb, NULL);
//        }
//    }

}
