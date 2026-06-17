//! Communication stuff

pub(crate) trait Bridge<Req, Res> {
    fn next_generation(&mut self) -> u32;
    fn new() -> Self;
    fn poll(&mut self) -> Res;
    fn send(&mut self) -> Req;
}
