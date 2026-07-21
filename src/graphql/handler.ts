import { graphql } from "graphql";
import { Elysia } from "elysia";
import { schema } from "./schema";

export const graphqlPlugin = new Elysia()
  .get("/graphql", async ({ query, set }) => {
    const q = query as { query?: string; variables?: string };
    if (!q.query) {
      set.status = 400;
      return { error: "Must provide query string" };
    }
    const result = await graphql({
      schema,
      source: q.query,
      variableValues: q.variables ? JSON.parse(q.variables) : undefined,
    });
    return result;
  }, {
    detail: { summary: "Execute GraphQL query (GET)", tags: ["GraphQL"] },
  })

  // ── POST /graphql ────────────────────────────────
  .post("/graphql", async ({ body, request, set }) => {
    const contentType = request.headers.get("content-type") || "";

    // Raw application/graphql
    if (contentType.includes("application/graphql")) {
      const text = await request.text();
      const result = await graphql({ schema, source: text });
      return result;
    }

    // JSON body
    const json = body as {
      query?: string;
      variables?: Record<string, unknown>;
      operationName?: string;
    };
    if (!json.query) {
      set.status = 400;
      return { error: "Must provide query string" };
    }
    const result = await graphql({
      schema,
      source: json.query,
      variableValues: json.variables,
      operationName: json.operationName,
    });
    return result;
  }, {
    detail: { summary: "Execute GraphQL query (POST)", tags: ["GraphQL"] },
  });
