
const args = process.argv.slice(2);
console.log("sample: args", args);
switch (args[0]) {
  case "greet":
    greet(args.slice(1)[0]);
    break;
  default:
    console.log("Unknown command");
    process.exitCode = 1;
}

export function greet(msg) {
  console.log("sample: greet", msg);
  switch (msg) {
    case "hello":
      console.log("Hello, World!");
      break;
    case "goodbye":
      console.log("Goodbye, World!");
      break;
    default:
      console.log("sample: Unknown command");
      process.exitCode = 2;
  }
}
