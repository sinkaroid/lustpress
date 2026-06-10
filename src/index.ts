import app from "./app";

// Vercel gerencia o servidor — não chame .listen() lá.
if (!process.env.VERCEL) {
  app.listen(process.env.PORT || 3000);
  console.log(
    `Lustpress is running at ${app.server?.hostname}:${app.server?.port}`
  );
}

export default app;
