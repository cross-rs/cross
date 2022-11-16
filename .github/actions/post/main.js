// adapted from https://github.com/actions/runner/issues/1478
const { exec } = require("child_process");

function run(cmd) {
  exec(cmd, { shell: "bash" }, (error, stdout, stderr) => {
    if (stdout.length != 0) {
      console.log(`${stdout}`);
    }
    if (stderr.length != 0) {
      console.error(`${stderr}`);
    }
    if (error) {
      process.exitCode = error.code;
      console.error(`${error}`);
    }
  });
}

if (process.env[`STATE_POST`] != undefined) {
  // Are we in the 'post' step?
  run(process.env.INPUT_POST);
} else {
  // Otherwise, this is the main step
  console.log(`POST=true >> $GITHUB_STATE`);
  run(process.env.INPUT_MAIN);
}
