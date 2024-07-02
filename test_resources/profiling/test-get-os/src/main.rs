mod imports;
use crate::imports::profiling::system::os;

fn main() {
    let os_info = os::info().unwrap();
    dbg!(os_info.distro_version);
    dbg!(os_info.kernel_version);
}
