#![no_std]
#![no_main]
extern crate panic_halt;


extern crate riscv;
extern crate riscv_rt;
use litex_pac;
use riscv_rt::entry;

use vexriscv::register::{vdci, vmim, vmip, vsim, vsip};
use riscv::register::{mstatus, mcause, mie};


mod ethernet;
mod print;
mod timer;
mod leds;
mod adc;
mod gpio;
mod gpio1;

use crate::ethernet::Eth;
use timer::Timer;
use timer::Timer2;
use leds::Leds;
use leds::Leds2;
use adc::Adc;
use gpio::Gpio;
use gpio1::Gpio1;

use managed::ManagedSlice;
use smoltcp::iface::{EthernetInterfaceBuilder, NeighborCache};
use smoltcp::socket::{
    SocketSet, TcpSocket, TcpSocketBuffer, UdpPacketMetadata, UdpSocket, UdpSocketBuffer,
};
use smoltcp::time::Duration;
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};

use crate::print::UartLogger;
use log::{debug, info, trace};



const SYSTEM_CLOCK_FREQUENCY: u32 = 49_600_000;



// interrupt::interrupt!(TIMER0, isr);

// fn isr(){
//     let a = 1 + 2;
// }

mod mock {
    use core::cell::Cell;
    use smoltcp::time::{Duration, Instant};

    #[derive(Debug)]
    pub struct Clock(Cell<Instant>);

    impl Clock {
        pub fn new() -> Clock {
            Clock(Cell::new(Instant::from_millis(0)))
        }

        pub fn advance(&self, duration: Duration) {
            self.0.set(self.0.get() + duration)
        }

        pub fn elapsed(&self) -> Instant {
            self.0.get()
        }
    }
}

// This is the entry point for the application.
// It is not allowed to return.
#[entry]
fn main() -> ! {
    let peripherals = litex_pac::Peripherals::take().unwrap();

    print::print_hardware::set_hardware(peripherals.UART);

    UartLogger::init().unwrap();

    info!("Logger initialized");

    let mut timer = Timer::new(peripherals.TIMER0);
    let mut timer2 = Timer2::new(peripherals.TIMER2);

    let mut leds = Leds::new(peripherals.LEDS);

    let mut adc = Adc::new(peripherals.ADC);

    let mut gpio = Gpio::new(peripherals.GPIO);

    let mut gpio1 = Gpio1::new(peripherals.GPIO1);

    let clock = mock::Clock::new();
    let device = Eth::new(peripherals.ETHMAC, peripherals.ETHMEM);

    let mut neighbor_cache_entries = [None; 8];
    let neighbor_cache = NeighborCache::new(&mut neighbor_cache_entries[..]);
    let mut ip_addr = [IpCidr::new(IpAddress::v4(192, 168, 1, 50), 24)];
    let ip_addrs = ManagedSlice::Borrowed(&mut ip_addr);
    let mut iface = EthernetInterfaceBuilder::new(device)
        .ethernet_addr(EthernetAddress::from_bytes(&[
            0xF6, 0x48, 0x74, 0xC8, 0xC4, 0x83,
        ]))
        .neighbor_cache(neighbor_cache)
        .ip_addrs(ip_addrs)
        .finalize();

    let tcp_server_socket = {
        // It is not strictly necessary to use a `static mut` and unsafe code here, but
        // on embedded systems that smoltcp targets it is far better to allocate the data
        // statically to verify that it fits into RAM rather than get undefined behavior
        // when stack overflows.
        #[link_section = ".main_ram"]
        static mut TCP_SERVER_RX_DATA: [u8; 8192] = [0; 8192];
        #[link_section = ".main_ram"]
        static mut TCP_SERVER_TX_DATA: [u8; 8192] = [0; 8192];
        let tcp_rx_buffer = TcpSocketBuffer::new(unsafe { &mut TCP_SERVER_RX_DATA[..] });
        let tcp_tx_buffer = TcpSocketBuffer::new(unsafe { &mut TCP_SERVER_TX_DATA[..] });
        TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer)
    };

    let udp_server_socket = {
        // It is not strictly necessary to use a `static mut` and unsafe code here, but
        // on embedded systems that smoltcp targets it is far better to allocate the data
        // statically to verify that it fits into RAM rather than get undefined behavior
        // when stack overflows.
        #[link_section = ".main_ram"]
        static mut UDP_SERVER_RX_METADATA: [UdpPacketMetadata; 32] = [UdpPacketMetadata::EMPTY; 32];
        #[link_section = ".main_ram"]
        static mut UDP_SERVER_TX_METADATA: [UdpPacketMetadata; 32] = [UdpPacketMetadata::EMPTY; 32];
        #[link_section = ".main_ram"]
        static mut UDP_SERVER_RX_DATA: [u8; 8192] = [0; 8192];
        #[link_section = ".main_ram"]
        static mut UDP_SERVER_TX_DATA: [u8; 8192] = [0; 8192];
        let udp_rx_buffer =
            UdpSocketBuffer::new(unsafe { &mut UDP_SERVER_RX_METADATA[..] }, unsafe {
                &mut UDP_SERVER_RX_DATA[..]
            });
        let udp_tx_buffer =
            UdpSocketBuffer::new(unsafe { &mut UDP_SERVER_TX_METADATA[..] }, unsafe {
                &mut UDP_SERVER_TX_DATA[..]
            });
        UdpSocket::new(udp_rx_buffer, udp_tx_buffer)
    };

    info!("Add sockets");

    let mut socket_set_entries: [_; 2] = Default::default();
    let mut socket_set = SocketSet::new(&mut socket_set_entries[..]);
    let tcp_server_handle = socket_set.add(tcp_server_socket);
    let udp_server_handle = socket_set.add(udp_server_socket);

    let mut vec: [u8; 5] = [1,2,3,4,5];

    // gpio.en_interrupt();
    // gpio1.en_interrupt();


    timer2.load(SYSTEM_CLOCK_FREQUENCY / 1_000 * 4000);

    timer2.enable();

    timer2.en_interrupt();


    info!("Main loop...");

    loop {
        // match iface.poll(&mut socket_set, clock.elapsed()) {
        //     Ok(_) => {}
        //     Err(e) => {
        //         debug!("Poll error: {}", e);
        //     }
        // }
        //
        // {
        //     let mut socket = socket_set.get::<TcpSocket>(tcp_server_handle);
        //
        //     if !socket.is_active() && !socket.is_listening() {
        //         info!("Start listen...");
        //         socket.listen(1234).unwrap();
        //     }
        //
        //     if socket.can_send() {
        //         info!("Can send...");
        //         socket.send_slice(&vec).unwrap();
        //     }
        //
        //
        //     gpio.set_interrupt_polarity(true);  // falling edge
        //     gpio.dis_interrupt();
        //     gpio1.set_interrupt_polarity(false);  // falling edge
        //     gpio1.dis_interrupt();
        //
        //     msleep(&mut timer, &mut leds, 1000 as u32);
            vec.rotate_right(1);
            // leds.toggle_mask(4);
            //
            // let adcval: u32 = adc.read();
            //
            // if socket.can_send() {
            //     socket.send_slice(&adcval.to_be_bytes()).unwrap();
            // }

            info!("\nbefore:");
            info!("vmip: {}", vmip::read());
            info!("vmim: {}", vmim::read());
            info!("mcause: {}", mcause::read().bits());
            info!("mie: {}", mie::read().bits());
            info!("btn: {}", gpio.status());
            info!("ev_pending_gpio: {}", gpio.ev_pending());
            info!("ev_status_gpio: {}", gpio.ev_status());
            info!("ev_enable_gpio: {}", gpio.ev_enable());
            info!("ev_pending_timer: {}", timer.ev_pending());
            info!("ev_status_timer: {}", timer.ev_status());
            info!("ev_enable_timer: {}", timer.ev_enable());


            //
            // timer.dis_interrupt();
            // timer.clr_interrupt();

            for i in 0..10000{
                for j in 0..1000{
                    leds.toggle_mask(2);    // toggle yellow led
                }
            }




            // timer.disable();

            // timer.reload(0);
            // timer.load(SYSTEM_CLOCK_FREQUENCY / 1_000 * 1000);

            // timer.enable();

            unsafe{
                vmim::write(0); // Disable all machine interrupts
                mie::set_msoft();
                // mie::set_mtimer();
                mie::set_mext();
                // mstatus::set_mie(); // Enable CPU interrupts
                vmim::write(0xFFFF_FFFF);   // 1010 for timer and gpio
                mstatus::set_mie();
            }
            // msleep(&mut timer, &mut leds, 500 as u32);
            info!("\nafter:");
            info!("vmip: {}", vmip::read());
            info!("vmim: {}", vmim::read());
            info!("mcause: {}", mcause::read().bits());
            info!("mie: {}", mie::read().bits());
            info!("btn: {}", gpio.status());
            info!("ev_pending_gpio: {}", gpio.ev_pending());
            info!("ev_status_gpio: {}", gpio.ev_status());
            info!("ev_enable_gpio: {}", gpio.ev_enable());
            info!("ev_pending_timer: {}", timer.ev_pending());
            info!("ev_status_timer: {}", timer.ev_status());
            info!("ev_enable_timer: {}", timer.ev_enable());
            // vmim::write(0);

            // info!("vsim: {}", vsim::read());
            // info!("vsip: {}", vsip::read());
        // }

        for i in 0..10000{
            for j in 0..1000{
                leds.toggle_mask(2);    // toggle yellow led
            }
        }
        leds.set(0x0);

        //
        // match iface.poll_delay(&socket_set, clock.elapsed()) {
        //     Some(Duration { millis: 0 }) => {}
        //     Some(delay) => {
        //         trace!("sleeping for {} ms", delay);
        //         msleep(&mut timer, &mut leds, delay.total_millis() as u32);
        //         clock.advance(delay)
        //     }
        //     None => clock.advance(Duration::from_millis(1)),
        // }
        // trace!("Clock elapsed: {}", clock.elapsed());


    }
}


#[no_mangle]
fn MachineExternal() {
    let mc = mcause::read();
    let irqs_pending = vmip::read();
    // vmim::write(0); // absolutely neccessary right now to disable interrupts otherwise the processor is stuck.

    if mc.is_exception() {};


    let peripherals = unsafe{ litex_pac::Peripherals::steal() };

    let mut timer = Timer::new(peripherals.TIMER0);
    let mut leds = Leds::new(peripherals.LEDS);
    let mut gpio1 = Gpio1::new(peripherals.GPIO1);
    let mut gpio = Gpio::new(peripherals.GPIO);
    gpio1.clr_interrupt();
    // gpio.clr_interrupt();




    if irqs_pending & (1 << 1) != 0 {
        handle_timer_irq();
    }

    if irqs_pending & (1 << 3) != 0 {
        handle_btn_irq();
    }

    if irqs_pending & (1 << 5) != 0 {
        handle_timer2_irq();
    }


    // timer.clr_interrupt();


}

fn handle_timer_irq() {

    let peripherals = unsafe{ litex_pac::Peripherals::steal() };

    let mut timer = Timer::new(peripherals.TIMER0);
    let mut leds = Leds::new(peripherals.LEDS);

    leds.toggle_mask(4);    // toggle green led
    timer.clr_interrupt();

}

fn handle_timer2_irq() {

    let peripherals = unsafe{ litex_pac::Peripherals::steal() };

    let mut timer2 = Timer2::new(peripherals.TIMER2);
    let mut leds2 = Leds2::new(peripherals.LEDS2);

    leds2.toggle_mask(0xf);
    timer2.clr_interrupt();
    timer2.disable();

    timer2.reload(0);

    timer2.load(SYSTEM_CLOCK_FREQUENCY / 1_000 * 4000);

    timer2.enable();

}

fn handle_btn_irq() {



    let peripherals = unsafe{ litex_pac::Peripherals::steal() };

    let mut gpio = Gpio::new(peripherals.GPIO);
    let mut leds = Leds::new(peripherals.LEDS);

    // gpio.clr_interrupt()
     leds.toggle_mask(0x10);
    // if gpio.ev_pending() == 0 {
    //     leds.toggle_mask(0x2);    // toggle green led
    // }
    // leds.toggle_mask(4);    // toggle green led
    // leds.toggle_mask(4);    // toggle green led
    for i in 0..10000{
        for j in 0..1045{
            leds.toggle_mask(0x8);    // toggle yellow led
        }
    }
    gpio.clr_interrupt();
    leds.toggle_mask(0xff);
    // if gpio.ev_pending() == 0 {
    //     leds.toggle_mask(0xff);    // toggle green led
    // }
    for i in 0..2000{
        for j in 0..1000{
            leds.toggle_mask(0x8);    // toggle yellow led
        }
    }
    // vmim::write(0);
    // gpio.dis_interrupt();
    // gpio.clr_interrupt();
    // gpio.en_interrupt();
    // gpio.clr_interrupt();
}




fn u32_to_u8(x:u32) -> [u8;4] {
    let b1 : u8 = ((x >> 24) & 0xff) as u8;
    let b2 : u8 = ((x >> 16) & 0xff) as u8;
    let b3 : u8 = ((x >> 8) & 0xff) as u8;
    let b4 : u8 = (x & 0xff) as u8;
    return [b1, b2, b3, b4]
}

fn msleep(timer: &mut Timer, leds : &mut Leds, ms: u32) {
    timer.disable();

    timer.reload(0);
    timer.load(SYSTEM_CLOCK_FREQUENCY / 1_000 * ms);

    timer.enable();

    // Wait until the time has elapsed
    while timer.value() > 0 {
        // leds.toggle_mask(2);    // toggle yellow led
    }
    timer.disable();
}