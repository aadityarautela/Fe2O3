extern crate minifb;
extern crate rand;
extern crate rodio;

use std::convert::TryInto;
use std::env;
use std::error::Error;
use std::fs;
use std::io::prelude::*;

use minifb::{Key, Scale, Window, WindowOptions};
use rand::prelude::*;
use rodio::Sink;

const MEMSIZE: usize = 4096;
const REG_COUNT: usize = 16;
const KEY_COUNT: usize = 16;
const DISPLAY_WIDTH: usize = 64;
const DISPLAY_HEIGHT: usize = 32;
const PX_ON: u32 = 0xefbb24;
const PX_OFF: u32 = 0x3c2f41;
const EMULOOP_TIMER: usize = 7;

//Utility function for hex numbers
//Returns number formed by first removing rem_count digits
//from the least significant place first, and then taking
//dgts_count digits.
//e.g. get_hex_dgts(0x1919,1,2) returns 0x9
fn get_hex_dgts(number: &u16, dgts_count: u32, rem_count: u32) -> usize {
    let base: u16 = 0x10;
    ((number / base.pow(rem_count)) % (base.pow(dgts_count))) as usize
}

//Utility function that tells whether a bit at position p from lsb is 1 or 0
fn get_bit(number: &u8, p: u8) -> u8 {
    if 1 & (number >> p) == 1 {
        1
    } else {
        0
    }
}

//Utility fn which handles keyboard events
fn handle_kb(window: &Window) -> Vec<bool> {
    let mut keys: Vec<bool> = vec![false;KEY_COUNT];
    window.get_keys().map(|keys_pressed| {
        for k in keys_pressed {
            match k {
                Key::Key1 => keys[0x1] = true,
                Key::Key2 => keys[0x2] = true,
                Key::Key3 => keys[0x3] = true,
                Key::Key4 => keys[0xc] = true,
                Key::Q => keys[0x4] = true,
                Key::W => keys[0x5] = true,
                Key::E => keys[0x6] = true,
                Key::R => keys[0xd] = true,
                Key::A => keys[0x7] = true,
                Key::S => keys[0x8] = true,
                Key::D => keys[0x9] = true,
                Key::F => keys[0xe] = true,
                Key::Z => keys[0xa] = true,
                Key::X => keys[0x0] = true,
                Key::C => keys[0xb] = true,
                Key::V => keys[0xf] = true,
                _ => (),
            };
        }
    });
    keys
}

fn main() {    
    //Audio Setup
    let au_dev = rodio::default_output_device().unwrap();
    let au_sink = Sink::new(&au_dev);
    let au_src = rodio::source::SineWave::new(466); //B flat
    au_sink.append(au_src);
    au_sink.pause();
    
    
    //Initializing CHIP8 System
    let mut opcode: u16 = 0;
    let mut memory: Vec<u8> = vec![0; MEMSIZE];
    let mut V: Vec<u8> = vec![0; REG_COUNT];
    let mut I: usize = 0;
    let mut pc: usize = 0x200;
    let mut display: Vec<u32> = vec![PX_OFF.try_into().unwrap(); DISPLAY_WIDTH * DISPLAY_HEIGHT];
    let mut delay_timer: u8 =0;
    let mut sound_timer: u8 =0;
    let mut stack: Vec<usize> = Vec::new();
    let mut key: Vec<bool> = vec![false; KEY_COUNT];
    
    //Load fontset
    let fontset = vec![
    0xf0, 0x90, 0x90, 0x90, 0xf0, 0x20, 0x60, 0x20, 0x20, 0x70, 0xf0, 0x10, 0xf0, 0x80, 0xf0,
    0xf0, 0x10, 0xf0, 0x10, 0xf0, 0x90, 0x90, 0xf0, 0x10, 0x10, 0xf0, 0x80, 0xf0, 0x10, 0xf0,
    0xf0, 0x80, 0xf0, 0x90, 0xf0, 0xf0, 0x10, 0x20, 0x40, 0x40, 0xf0, 0x90, 0xf0, 0x90, 0xf0,
    0xf0, 0x90, 0xf0, 0x10, 0xf0, 0xf0, 0x90, 0xf0, 0x90, 0x90, 0xe0, 0x90, 0xe0, 0x90, 0xe0,
    0xf0, 0x80, 0x80, 0x80, 0xf0, 0xe0, 0x90, 0x90, 0x90, 0xe0, 0xf0, 0x80, 0xf0, 0x80, 0xf0,
    0xf0, 0x80, 0xf0, 0x80, 0x80,
    ];
    
    for i in 0..80 {
        memory[i] = fontset[i];
    }
    
    
    //Loading game to memory
    let args: Vec<String> = env::args().collect();
    let mut filename: String;
    if args.len() == 1 {
        panic!("File not Specified");
    } else {
        filename = String::from(&args[1]);
    }
    
    let rom = match fs::read(&filename) {
        Err(e) => panic!("Can't open file: {}", e.to_string()),
        Ok(file) => file,
    };
    
    for (i, j) in rom.into_iter().enumerate() {
        if i >= MEMSIZE {
            panic!("Memory limit exceeded");
        }
        println!("Byte {:02}: {:#04x}", i, j);
        memory[i + 512] = j.try_into().unwrap();
    }
    
    //Graphics Setup
    let mut window = Window::new(
        &format!("Fe2O3: {}", filename),
        DISPLAY_WIDTH,
        DISPLAY_HEIGHT,
        WindowOptions {
            scale: Scale::X16,
            ..WindowOptions::default()
        },
    )
    .unwrap();
    //420Hz
    window.limit_update_rate(Some(std::time::Duration::from_micros(2381)));
    
    
    let mut waiting_for_input = false;
    let mut keypress_reg: usize = 0;
    let mut isexec = true;
    let mut exec_next = true;
    let mut emuloop_t = EMULOOP_TIMER;
    
    while window.is_open() && !window.is_key_down(Key::Escape) && pc <= MEMSIZE {
        key = handle_kb(&window);
        for (i,j) in key.iter().enumerate() {
            if *j{
                if waiting_for_input == true{ 
                    isexec = true;
                    waiting_for_input = false;
                    V[keypress_reg] = i as u8;
                    break;
                }
                println!("{:01x} key pressed",i);
            }
        }
        
        
        //Getting 2 bytes out of memory
        opcode = ((memory[pc] as u16)*256) as u16 + memory[pc + 1] as u16;
        
        //Execution loop
        if isexec{
            //Printing Opcode
            println!("pc={:03x}, opcode={:04x}",pc,opcode);
            
            //Let opcode be represented as lmno where l,m,n,o in hex
            match opcode{
                0x00e0 => {
                    //Clear Display
                    for i in 0..display.len() {
                        display[i] = PX_OFF.try_into().unwrap();
                    }
                    exec_next = true;
                }
                0x00ee => {
                    //Returns from a subroutine
                    pc = stack.pop().expect("Stack Empty");
                    exec_next = true;
                }
                0x1000..=0x1fff => {
                    //Jumps to mno
                    pc = get_hex_dgts(&opcode, 3, 0);
                    exec_next = false;
                }
                0x2000..=0x2fff => {
                    //Call Subroutine at mno
                    stack.push(pc);
                    pc = get_hex_dgts(&opcode, 3, 0);
                    exec_next = false;
                }
                0x3000..=0x3fff => {
                    //Skip next instruction if Vm = no
                    if V[get_hex_dgts(&opcode, 1, 2)] == get_hex_dgts(&opcode, 2, 0) as u8 {
                        pc += 2;
                    }
                    exec_next = true;
                }
                0x4000..=0x4fff => {
                    //Skip next instruction if Vx != no
                    if V[get_hex_dgts(&opcode, 1, 2)] != get_hex_dgts(&opcode, 2, 0) as u8 {
                        pc += 2;
                    }
                    exec_next = true;
                }
                0x5000..=0x5fff => {
                    //Skip next instruction if Vm = Vn
                    if V[get_hex_dgts(&opcode, 1, 1)] == V[get_hex_dgts(&opcode, 1, 2)] {
                        pc += 2;
                    }
                    exec_next = true;
                }
                0x6000..=0x6fff => {
                    //Set Vm = n
                    V[get_hex_dgts(&opcode, 1, 2)] = get_hex_dgts(&opcode, 2, 0) as u8 ;
                    exec_next = true;
                }
                0x7000..=0x7fff => {
                    //Set Vm = Vm + no
                    let tmp_m = get_hex_dgts(&opcode, 1, 2);
                    let tmp_no = get_hex_dgts(&opcode, 2, 0);
                    V[tmp_m] = V[tmp_m].overflowing_add(tmp_no as u8).0;
                    /* V[get_hex_dgts(&opcode, 1, 2)] = (V[get_hex_dgts(&opcode, 1, 2)]
                    .overflowing_add(get_hex_dgts(&opcode, 2, 0) as u8).0 as usize; */
                    exec_next = true;
                }
                0x8000..=0x8fff => {
                    //Since this has many subcases with variations in o, we use another match case
                    let lsb = get_hex_dgts(&opcode, 1, 0);
                    let m = get_hex_dgts(&opcode, 1, 2);
                    let n = get_hex_dgts(&opcode, 1, 1);
                    
                    match lsb {
                        0x0 => {
                            //Set Vm = Vn
                            V[m] = V[n];
                        }
                        0x1 => {
                            //Set Vm = Vm OR Vn
                            V[m] = V[m] | V[n];
                        }
                        0x2 => {
                            //Set Vm = Vm AND Vn
                            V[m] = V[m] & V[n];
                        }
                        0x3 => {
                            //Set Vm = Vm XOR Vn
                            V[m] = V[m] ^ V[n];
                        }
                        0x4 => {
                            //Set Vm = Vm + Vn, set Vf = carry if overflow
                            let tmp = V[m].overflowing_add(V[n] as u8).0;
                            let is_overflow = (V[m] as u8).overflowing_add(V[n] as u8).1 as bool;
                            V[m] = tmp;
                            V[0xf] = if is_overflow { 1 } else { 0 };
                        }
                        0x5 => {
                            //Set Vm = Vm - Vn, set Vf = NOT borrow
                            let (tmp,is_borrow) = V[m].overflowing_sub(V[n]);
                            V[m] = tmp;
                            V[0xf] = if is_borrow { 0 } else { 1 };
                        }
                        0x6 => {
                            //Set Vm = Vm SHR 1, If the least-significant bit of Vm is 1, then VF is set to 1, otherwise 0.
                            V[0xf] = get_bit(&V[m], 0);
                            let tmp = V[m].overflowing_shr(1).0;
                            V[m] = tmp;
                        }
                        0x7 => {
                            //Set Vm = Vn - Vm, set Vf = NOT borrow
                            let (tmp, is_borrow) = V[n].overflowing_sub(V[m]);
                            V[m] = tmp;
                            V[0xf] = if is_borrow { 0 } else { 1 };
                        }
                        0xe => {
                            //Set Vm = Vm SHL 1. If the most-significant bit of Vm is 1, then Vf is set to 1, otherwise to 0.
                            V[0xf] = get_bit(&V[m], 7);
                            let tmp = V[m].overflowing_shl(1).0;
                            V[m] = tmp;
                        }
                        _ => {
                            println!("Warning! Instruction Not Recognised");
                        }
                    };
                    exec_next = true;
                }
                0x9000..=0x9fff => {
                    //Skip next instruction if Vm != Vn
                    if V[get_hex_dgts(&opcode, 1, 2)] != V[get_hex_dgts(&opcode, 1, 1)] {
                        pc += 2;
                    }
                    exec_next = true;
                }
                0xa000..=0xafff => {
                    // Set I = mno
                    I = get_hex_dgts(&opcode, 3, 0) ;
                    exec_next = true;
                }
                0xb000..=0xbfff => {
                    //Jump to location mno + V0
                    pc = (V[0] + (get_hex_dgts(&opcode, 3, 0) as u8)) as usize ;
                    exec_next = false;
                }
                
                0xc000..=0xcfff => {
                    //Set Vm = random byte AND no
                    let rnd = rand::random::<u8>();
                    V[get_hex_dgts(&opcode, 1, 2)] = (rnd & (get_hex_dgts(&opcode, 2, 0) as u8)) as u8 ;
                    exec_next = true;
                }
                0xd000..=0xdfff => {
                    //The interpreter reads o bytes from memory, starting at the address stored in I.
                    //These bytes are then displayed as sprites on screen at coordinates (Vm, Vn).
                    //Sprites are XORed onto the existing screen.
                    //If this causes any pixels to be erased, Vf is set to 1, otherwise it is set to 0.
                    //If the sprite is positioned so part of it is outside the coordinates of the display
                    //it wraps around to the opposite side of the screen.
                    let reg_m = get_hex_dgts(&opcode,1,2);
                    let reg_n = get_hex_dgts(&opcode,1,1);
                    let init_x = V[reg_m];
                    let init_y = V[reg_n];
                    let mut byte_cnt = get_hex_dgts(&opcode,1,0);
                    let mut bytes_to_print: Vec<usize>  = Vec::new();
                    let mut did_collision_happen = 0;
                    let mut c = 0;
                    while byte_cnt > 0{
                        bytes_to_print.push(memory[(I+c)] as usize);
                        byte_cnt -= 1;
                        c += 1;
                    }
                    
                    for (a,b) in  bytes_to_print.iter().enumerate() {
                        for c in 0..8{
                            //Taking care of warping
                            let x = ((init_x.overflowing_add(c).0) as usize)%DISPLAY_WIDTH;
                            let y = ((init_y.overflowing_add(a as u8).0) as usize)%DISPLAY_HEIGHT;
                            let coordinate = (y*DISPLAY_WIDTH) + x;                
                            if get_bit(&(*b as u8), (8-c-1) as u8) == 1 {
                                if display[coordinate] == (PX_ON.try_into().unwrap()) {
                                    did_collision_happen = 1;
                                    display[coordinate] = PX_OFF.try_into().unwrap();
                                }
                                else {
                                    display[coordinate] = PX_ON.try_into().unwrap();
                                }
                            }
                            V[0xf] = did_collision_happen;
                        }
                    }
                    exec_next = true;
                }
                0xe000..=0xefff => {
                    let ltsd = get_hex_dgts(&opcode, 2, 0);
                    match ltsd{
                        0x9e => {
                            //Skip next instruction if key with the value of Vm is pressed.
                            let tmp_m = get_hex_dgts(&opcode, 1, 2);
                            if key[V[tmp_m] as usize] != false {
                                pc += 2;
                            }
                        }
                        0xa1 => {
                            //Skip next instruction if key with the value of Vm is NOT pressed.
                            let tmp_m = get_hex_dgts(&opcode, 1, 2);
                            if key[V[tmp_m] as usize] == false {
                                pc += 2;
                            }
                        }
                        _ => {
                            println!("Warning! Opcode unrecognized");
                        }
                    };
                    exec_next = true;
                }
                0xf000..=0xffff => {
                    let ltsd = get_hex_dgts(&opcode, 2, 0);
                    match ltsd{
                        0x07 => {
                            //Set Vm = delay timer value.
                            let tmp_m = get_hex_dgts(&opcode, 1, 2);
                            V[tmp_m] = delay_timer;
                        }
                        0x0a => {
                            //Wait for a key press, store the value of the key in Vm.
                            let tmp_m = get_hex_dgts(&opcode, 1, 2);
                            isexec = false;
                            waiting_for_input = true;
                            keypress_reg = tmp_m
                        }
                        0x15 => {
                            //Set delay timer val = Vm
                            let tmp_m = get_hex_dgts(&opcode, 1, 2);
                            delay_timer = V[tmp_m];
                        }
                        0x18 => {
                            //Set sound timer = Vm
                            let tmp_m = get_hex_dgts(&opcode, 1, 2);
                            sound_timer = V[tmp_m];
                        }
                        0x1e => {
                            //Set I = I + Vm
                            let tmp_m = get_hex_dgts(&opcode, 1, 2);
                            I += V[tmp_m] as usize;
                        }
                        0x29 => {
                            //Set I = location of sprite for digit Vm.
                            let tmp_m = get_hex_dgts(&opcode, 1, 2);
                            I = (V[tmp_m] as usize) * 0x5;
                        }
                        0x33 => {
                            //Store BCD representation of Vm in memory locations I, I+1, and I+2. 
                            //Places the hundreds digit in memory at location in I, the tens digit at location I+1,
                            //and the ones digit at location I+2.
                            let tmp_m  = get_hex_dgts(&opcode, 1,2);
                            memory[I] = V[tmp_m] / 100;
                            memory[I + 1] = (V[tmp_m] % 100) / 10;
                            memory[I + 2] = V[tmp_m] % 10;
                        }
                        0x55 => {
                            //Store registers V0 through Vx in memory starting at location I
                            for c in 0..16 {
                                memory[I + c] = V[c];
                            }
                        }
                        0x65 => {
                            //Read registers V0 through Vx from memory starting at location I.
                            for c in 0..16 {
                                V[c] = memory[I + c];
                            }
                        }
                        _ => {
                            println!("Warning! Opcode unrecognized");
                        }
                    };
                    exec_next = true;
                    
                }
                _ => {
                    println!("Warning! Opcode unrecognized");
                    exec_next = true;
                }
            };
            if exec_next != false {
                pc += 2;
            }
        }
        if emuloop_t ==0 {         
            if delay_timer > 0 {
                delay_timer -= 1;
            }
            if sound_timer > 0 {
                //Play Sound
                au_sink.play();
                sound_timer -= 1;
            }
            if sound_timer == 0 {
                //Pause Sound
                au_sink.pause();
            }
            emuloop_t = EMULOOP_TIMER;
            window.update_with_buffer(&display, DISPLAY_WIDTH, DISPLAY_HEIGHT);
        }
        else{
            emuloop_t -=1
        }
    }
} 