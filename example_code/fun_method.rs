trait T {
    fn bla(&self){let g=3;}
}

struct S;

impl S {
    fn met(&self) {
        let k = 44;
    }
}

impl T for S {
    fn bla(&self) {
        let i = 6;
    }
}

struct R;

impl T for R {
    fn bla(&self) {
        let x = 4;
    }
}

fn virt(ob: &dyn T) {
    ob.bla();
}

fn main() {
    let s = S {};
    s.met();
    s.bla();
}
