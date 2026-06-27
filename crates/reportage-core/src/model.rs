#[derive(Debug)]
pub struct Script {
    pub cases: Vec<Case>,
}

#[derive(Debug)]
pub struct Case {
    pub name: String,
    pub steps: Vec<Step>,
}

#[derive(Debug)]
pub enum Step {
    Action(Action),
    Assert(AssertStep),
}

#[derive(Debug)]
pub struct Action {
    pub command: String,
}

#[derive(Debug)]
pub enum AssertStep {
    Exit { expected: u8 },
}
