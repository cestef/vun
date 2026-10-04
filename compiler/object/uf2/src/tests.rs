use super::*;

/// The family of the RP2350's Arm programs in the secure state, as the format's registry has it.
const RP2350_ARM_SECURE: u32 = 0xe48b_ff59;

#[test]
fn pieces_are_cut_into_blocks_that_read_back_as_they_were_made() {
    let flash: Vec<u8> = (0..600).map(|b| b as u8).collect();
    let ram = [7; 10];
    let pieces = [Piece { address: 0x1000_0000, bytes: &flash }, Piece { address: 0x2000_0000, bytes: &ram }];
    let file = Uf2 { family: RP2350_ARM_SECURE }.write(&pieces).unwrap();
    assert_eq!(file.len(), 4 * Block::SIZE);
    let (uf2, blocks) = Uf2::read(&file).unwrap();
    assert_eq!(uf2.family, RP2350_ARM_SECURE);
    let placed: Vec<(u32, usize)> = blocks.iter().map(|b| (b.address, b.payload.len())).collect();
    assert_eq!(placed, [(0x1000_0000, 256), (0x1000_0100, 256), (0x1000_0200, 88), (0x2000_0000, 10)]);
    assert_eq!(blocks[1].payload, flash[256..512]);
}

#[test]
fn an_address_past_32_bits_is_refused() {
    let piece = Piece { address: 0x1_0000_0000, bytes: &[0] };
    assert_eq!(Uf2 { family: 0 }.write(&[piece]), Err(Unaddressable { address: 0x1_0000_0000 }));
}

#[test]
fn a_file_breaking_the_format_is_refused_where_it_does() {
    let file = Uf2 { family: 1 }.write(&[Piece { address: 0, bytes: &[0; 300] }]).unwrap();
    assert_eq!(Uf2::read(&file[..Block::SIZE + 1]), Err(Malformed::Truncated));
    let mut magic = file.clone();
    magic[Block::SIZE] ^= 1;
    assert_eq!(Uf2::read(&magic), Err(Malformed::Magic { block: 1 }));
    let mut numbered = file.clone();
    numbered[Block::SIZE + 20] = 5;
    assert_eq!(Uf2::read(&numbered), Err(Malformed::Numbering { block: 1 }));
    let mut other = file;
    other[Block::SIZE + 28] = 2;
    assert_eq!(Uf2::read(&other), Err(Malformed::Family { block: 1 }));
}
