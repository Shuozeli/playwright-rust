'use strict';

const { setTimeout: schedule, clearTimeout: cancel } = require('node:timers');

// One Node-event-loop deadline shared by every later phase of the same public
// call. Rust owns ticket revocation; Node owns delivery timing and the raw
// timeout value's JavaScript timer semantics. In particular, synchronous user
// code may run past the nominal deadline before this callback can execute.
function createProgress(preparation, timeout) {
  let timer;
  let deadline;
  if (timeout) {
    const error = new Error(`PLAYWRIGHT_RUST_TIMEOUT: Timeout ${timeout}ms exceeded`);
    error.code = 'PLAYWRIGHT_RUST_TIMEOUT';
    deadline = new Promise((_, reject) => {
      timer = schedule(() => {
        timer = undefined;
        preparation.abortDeadline();
        reject(error);
      }, timeout);
    });
  }
  return Object.freeze({
    race(value) {
      return deadline ? Promise.race([value, deadline]) : Promise.resolve(value);
    },
    close() {
      if (timer !== undefined) {
        cancel(timer);
        timer = undefined;
      }
    },
  });
}

module.exports = { createProgress };
