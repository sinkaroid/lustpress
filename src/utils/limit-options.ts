import rateLimit from "express-rate-limit";
import slowDown from "express-slow-down";

const limiter = rateLimit({
  windowMs: 15 * 60 * 1000,
  max: 50,
  message: "Too nasty, please slow down"
});

const slow = slowDown({
  delayAfter: 50,
  windowMs: 15 * 60 * 1000,
  delayMs: 1000,
  maxDelayMs: 20000,
});

export { limiter, slow };