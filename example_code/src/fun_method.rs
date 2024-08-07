pub trait T {
    fn bla(&self){let _g=3;}
}

pub struct S;

impl S {
    pub fn met(&self) {
        let _k = 44;
    }
}

impl T for S {
    fn bla(&self) {
        let _i = 6;
    }
}

pub struct R;

impl T for R {
     fn bla(&self) {
        let _x = 4;
    }
}

pub fn _virt(ob: &dyn T) {
    ob.bla();
}
