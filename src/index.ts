import "dotenv/config";
import LustPress from "./LustPress";
import express from "express";
import { Request, Response, NextFunction } from "express";
import scrapeRoutes from "./router/endpoint";
import { slow, limiter } from "./utils/limit-options";
import { logger } from "./utils/logger";
import * as pkg from "../package.json";

const lust = new LustPress();
const app = express();


app.get("/", slow, limiter, async (req, res) => {
  res.send({
    success: true,
    playground: "https://sinkaroid.github.io/lustpress",
    endpoint: "https://github.com/sinkaroid/lustpress/blob/master/README.md#routing",
    date: new Date().toLocaleString(),
    rss: lust.currentProccess().rss,
    heap: lust.currentProccess().heap,
    server: await lust.getServer(),
    version: `${pkg.version}`,
  });
  logger.info({
    path: req.path,
    method: req.method,
    ip: req.ip,
    useragent: req.get("User-Agent")
  });
});

app.use(scrapeRoutes());
app.use((req: Request, res: Response, next: NextFunction) => {
  res.status(404);
  next(Error(`The page not found in path ${req.url} and method ${req.method}`));
  logger.error({
    path: req.url,
    method: req.method,
    ip: req.ip,
    useragent: req.get("User-Agent")
  });
});

app.use((error: any, res: Response) => {
  res.status(500).json({
    message: error.message,
    stack: error.stack
  });
});

app.listen(process.env.PORT || 3000, () => console.log(`${pkg.name} is running on port ${process.env.PORT || 3000}`));