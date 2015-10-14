pub enum Chunk<O> {
    Chunk(O),
    Flush,
    End
}

impl<O> Chunk<O> {

    pub fn map<P, F: FnOnce(O) -> P>(self, f: F) -> Chunk<P> {
        match self {
            Chunk::Chunk(o) => Chunk::Chunk(f(o)),
            Chunk::Flush => Chunk::Flush,
            Chunk::End => Chunk::End
        }
    }

    pub fn unwrap_or(self, alternative: O) -> O {
        match self {
            Chunk::Chunk(o) => o,
            _ => alternative
        }
    }

}

impl<O> From<O> for Chunk<O> {
    fn from(o: O) -> Chunk<O> {
        Chunk::Chunk(o)
    }
}
