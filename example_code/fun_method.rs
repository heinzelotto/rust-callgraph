struct S;

impl S {
    fn met(&self) {
        let k = 44;
    }
}

trait T {
    fn bla(&self);
}

impl T for S {
    fn bla(&self) {
        let i = 6;
    }
}

fn main() {
    let s = S {};
    s.met();
    s.bla();
}
