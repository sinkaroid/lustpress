import { describe, expect, it, beforeAll, afterAll } from "bun:test";
import { Elysia } from "elysia";
import { graphqlPlugin } from "../src/graphql/handler";

const port = 9876;
const url = `http://localhost:${port}/graphql`;

let app: ReturnType<typeof newElysia>;

function newElysia() {
  return new Elysia().use(graphqlPlugin);
}

beforeAll(() => {
  app = newElysia().listen(port);
});

afterAll(() => {
  app?.stop();
});

describe("GraphQL", () => {
  it("POST — returns error for missing query", async () => {
    const res = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({}),
    });
    expect(res.status).toBe(400);
    const json = await res.json();
    expect(json.error).toBe("Must provide query string");
  });

  it("GET — returns error for missing query", async () => {
    const res = await fetch(url);
    expect(res.status).toBe(400);
    const json = await res.json();
    expect(json.error).toBe("Must provide query string");
  });

  it("POST raw application/graphql", async () => {
    const res = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/graphql" },
      body: "{ __typename }",
    });
    expect(res.status).toBe(200);
    const json = await res.json();
    expect(json.data).toBeDefined();
  });

  it("POST JSON — resolves a query", async () => {
    const res = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        query: "{ __typename }",
      }),
    });
    expect(res.status).toBe(200);
    const json = await res.json();
    expect(json.data.__typename).toBe("Query");
  });
});
