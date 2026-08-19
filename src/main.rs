// Probe: does the C route build on this target, and does it decode to the SAME
// bytes as x86? Contains no Wrap code — only the two questions we cannot answer
// on a Windows machine.
//
// Prints one FNV-1a hash per decoded image. Two platforms agreeing on every
// hash is what "the asset identity does not depend on the client's machine"
// means in practice.

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// A tiny JPEG built at runtime so the probe needs no binary fixture in git.
/// Encoded by turbojpeg itself, then decoded back — the round trip exercises
/// the SIMD paths we care about.
fn make_jpeg(w: i32, h: i32) -> Vec<u8> {
    let mut rgb = vec![0u8; (w * h * 3) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 3) as usize;
            // Gradients plus a hard edge: smooth areas exercise the DCT,
            // the edge exercises chroma upsampling.
            rgb[i] = (x * 255 / w) as u8;
            rgb[i + 1] = (y * 255 / h) as u8;
            rgb[i + 2] = if (x / 8 + y / 8) % 2 == 0 { 240 } else { 16 };
        }
    }
    unsafe {
        let handle = turbojpeg_sys::tj3Init(turbojpeg_sys::TJINIT_TJINIT_COMPRESS as i32);
        assert!(!handle.is_null(), "tj3Init compress");
        turbojpeg_sys::tj3Set(
            handle,
            turbojpeg_sys::TJPARAM_TJPARAM_QUALITY as i32,
            90,
        );
        turbojpeg_sys::tj3Set(
            handle,
            turbojpeg_sys::TJPARAM_TJPARAM_SUBSAMP as i32,
            turbojpeg_sys::TJSAMP_TJSAMP_420 as i32,
        );
        let mut buf: *mut u8 = std::ptr::null_mut();
        let mut size: turbojpeg_sys::size_t = 0;
        let rc = turbojpeg_sys::tj3Compress8(
            handle,
            rgb.as_ptr(),
            w,
            0,
            h,
            turbojpeg_sys::TJPF_TJPF_RGB as i32,
            &mut buf,
            &mut size,
        );
        assert_eq!(rc, 0, "tj3Compress8");
        let out = std::slice::from_raw_parts(buf, size as usize).to_vec();
        turbojpeg_sys::tj3Free(buf as *mut std::ffi::c_void);
        turbojpeg_sys::tj3Destroy(handle);
        out
    }
}

fn decode(jpeg: &[u8], scale_num: i32, scale_denom: i32) -> (i32, i32, Vec<u8>) {
    unsafe {
        let handle = turbojpeg_sys::tj3Init(turbojpeg_sys::TJINIT_TJINIT_DECOMPRESS as i32);
        assert!(!handle.is_null(), "tj3Init decompress");
        let rc = turbojpeg_sys::tj3DecompressHeader(handle, jpeg.as_ptr(), jpeg.len() as _);
        assert_eq!(rc, 0, "tj3DecompressHeader");
        if scale_denom != 1 {
            let f = turbojpeg_sys::tjscalingfactor {
                num: scale_num,
                denom: scale_denom,
            };
            assert_eq!(
                turbojpeg_sys::tj3SetScalingFactor(handle, f),
                0,
                "tj3SetScalingFactor {scale_num}/{scale_denom}"
            );
        }
        // TurboJPEG 3 has no SCALEDWIDTH parameter: the scaled size is the
        // TJSCALED() macro applied to the full size, and it rounds UP.
        let jw = turbojpeg_sys::tj3Get(handle, turbojpeg_sys::TJPARAM_TJPARAM_JPEGWIDTH as i32);
        let jh = turbojpeg_sys::tj3Get(handle, turbojpeg_sys::TJPARAM_TJPARAM_JPEGHEIGHT as i32);
        let scaled = |d: i32| (d * scale_num + scale_denom - 1) / scale_denom;
        let (w, h) = (scaled(jw), scaled(jh));
        let mut out = vec![0u8; (w * h * 3) as usize];
        let rc = turbojpeg_sys::tj3Decompress8(
            handle,
            jpeg.as_ptr(),
            jpeg.len() as _,
            out.as_mut_ptr(),
            0,
            turbojpeg_sys::TJPF_TJPF_RGB as i32,
        );
        assert_eq!(rc, 0, "tj3Decompress8");
        turbojpeg_sys::tj3Destroy(handle);
        (w, h, out)
    }
}

unsafe extern "C" {
    fn jpegli_quality_to_distance(quality: i32) -> f32;
}

fn main() {
    println!("cible      {}", std::env::consts::ARCH);
    println!("systeme    {}", std::env::consts::OS);
    println!(
        "ABI        size_t du binding = {} octets, usize = {} octets",
        std::mem::size_of::<turbojpeg_sys::size_t>(),
        std::mem::size_of::<usize>()
    );

    // 1. Le binaire lie-t-il jpegli (C++) sur cette cible ?
    print!("jpegli     quality->distance :");
    for q in [50, 75, 90, 95] {
        print!(" {:.4}", unsafe { jpegli_quality_to_distance(q) });
    }
    println!();

    // 2. Les octets decodes sont-ils les memes que sur x86 ?
    //    Dimensions volontairement non rondes : un multiple de 8 alignerait
    //    les blocs et masquerait les differences de bord.
    let jpeg = make_jpeg(637, 419);
    println!("jpeg       {} octets, fnv {:016x}", jpeg.len(), fnv1a(&jpeg));
    for (num, denom) in [(1, 1), (1, 2), (1, 4), (1, 8), (3, 8), (7, 8)] {
        let (w, h, px) = decode(&jpeg, num, denom);
        println!(
            "decode     {num}/{denom}  {w}x{h}  fnv {:016x}",
            fnv1a(&px)
        );
    }
}
