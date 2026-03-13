import test from "node:test";
import assert from "node:assert/strict";
import p from "phin";

const port = process.env.PORT ?? 3000;

type ApiResponse = {
  success: boolean;
  data?: {
    id?: string | number;
  };
};

async function run(path: string) {
  const res = await p({
    url: `http://localhost:${port}${path}`,
    parse: "json",
    timeout: 20000
  });

  assert.equal(res.statusCode, 200);

  const json = res.body as ApiResponse;

  console.log(JSON.stringify(json, null, 2));

  if (!json.success) {
    throw new Error("scraper failed");
  }
}

test("pornhub", async () => {
  await run("/pornhub/random");
});

test("xnxx", async () => {
  await run("/xnxx/random");
});

test("redtube", async () => {
  await run("/redtube/random");
});

test("xvideos", async () => {
  await run("/xvideos/random");
});

test("xhamster", async () => {
  await run("/xhamster/random");
});

test("youporn", async () => {
  await run("/youporn/random");
});

test("eporner", async () => {
  await run("/eporner/random");
});

test("txxx", async () => {
  await run("/txxx/random");
});