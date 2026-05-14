import { Elysia } from "elysia";
import { cors } from "@elysiajs/cors";
import { swagger } from "@elysiajs/swagger";
import { lust } from "./LustPress";
import { scrapeRoutes } from "./router/endpoint";
import pkg from "../package.json";

const app = new Elysia()
  .use(cors())
  .use(
    swagger({
      path: "/docs",
      documentation: {
        info: {
          title: "Lustpress API",
          version: pkg.version,
          description: pkg.description,
        },
      },
    })
  )
  .get("/", async () => ({
    success: true,
    playground: "https://sinkaroid.github.io/lustpress",
    endpoint:
      "https://github.com/sinkaroid/lustpress/blob/master/README.md#routing",
    date: new Date().toLocaleString(),
    rss: lust.currentProccess().rss,
    heap: lust.currentProccess().heap,
    server: await lust.getServer(),
    version: pkg.version,
  }))
  .onError(({ code, error, set }) => {
    if (code === "NOT_FOUND") {
      set.status = 404;
      return {
        success: false,
        message: (error as Error).message || "Not Found",
      };
    }
    if (code === "VALIDATION") {
      set.status = 400;
      return {
        success: false,
        message: JSON.parse((error as Error).message)[0]?.message || "Validation Error",
      };
    }
    if (code === "UNKNOWN") {
      set.status = 400;
      return {
        success: false,
        message: (error as Error).message,
      };
    }
    set.status = 500;
    return {
      success: false,
      message: (error as Error).message || "Internal Server Error",
      stack: (error as Error).stack,
    };
  })
  .use(scrapeRoutes) // reuse legacy error handling
  .listen(process.env.PORT || 3000);

console.log(
  `Lustpress is running at ${app.server?.hostname}:${app.server?.port}`
);

export type App = typeof app;
