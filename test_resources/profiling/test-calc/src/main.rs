#[allow(warnings)]
mod imports;

use imports::profiling::calc::calc_it::Calculator;

use self::imports::profiling::calc::calc_it::Op;

fn main() {
    let res = Calculator::calc(Op::Add, 1, 1);
    assert_eq!(res, 2);
    println!("1 + 1 = {res}");
    let res = Calculator::calc(Op::Sub, 1, 1);
    assert_eq!(res, 0);
    println!("1 - 1 = {res}");
}
