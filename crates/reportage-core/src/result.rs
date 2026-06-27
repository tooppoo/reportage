#[derive(Debug)]
pub struct ActionOutput {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug)]
pub enum AssertionKind {
    Exit { expected: u8, actual: i32 },
}

#[derive(Debug)]
pub struct AssertionResult {
    pub step_index: usize,
    pub target_action_index: usize,
    pub kind: AssertionKind,
    pub passed: bool,
}

#[derive(Debug)]
pub enum CaseStatus {
    Pass,
    Fail,
    ValidationError(String),
    RuntimeError(String),
}

#[derive(Debug)]
pub struct CaseResult {
    pub name: String,
    pub status: CaseStatus,
    pub actions: Vec<ActionOutput>,
    pub assertions: Vec<AssertionResult>,
}

#[derive(Debug)]
pub struct RunResult {
    pub cases: Vec<CaseResult>,
}

impl RunResult {
    pub fn exit_code(&self) -> i32 {
        self.cases.iter().fold(0i32, |max, case| {
            let code = match &case.status {
                CaseStatus::Pass => 0,
                CaseStatus::Fail => 1,
                CaseStatus::ValidationError(_) => 2,
                CaseStatus::RuntimeError(_) => 3,
            };
            max.max(code)
        })
    }
}
