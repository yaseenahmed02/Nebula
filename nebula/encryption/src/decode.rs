extern crate steganography;
use steganography::decoder::*;
use steganography::util::*;

fn main() {
    let encoded_image = file_as_image_buffer("encodedIMG.png".to_string());
    let dec = Decoder::new(encoded_image);

    let out_buffer = dec.decode_alpha(); //decode alpha channel
    let clean_buffer: Vec<u8> = out_buffer.into_iter().filter(|b| {*b != 0xff_u8}).collect(); //filter fully opaque pixels to get alpha values that have mssg
    
    let message = bytes_to_str(clean_buffer.as_slice());
    println!("{:?}", message);
}
