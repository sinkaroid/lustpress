import { Request, Response } from "express";
import { scrapeContent } from "../../scraper/pornhub/pornhubGetController";
import c from "../../utils/options";
import { logger } from "../../utils/logger";
import { maybeError } from "../../utils/modifier";

export async function randomPornhub(req: Request, res: Response) {
  try {
    /**
     * @api {get} /pornhub/random Random pornhub video
     * @apiName Random pornhub
     * @apiGroup pornhub
     * @apiDescription Gets random pornhub video
     * 
     * @apiSuccessExample {json} Success-Response:
     *   HTTP/1.1 200 OK
     *   HTTP/1.1 400 Bad Request
     * 
     * @apiExample {curl} curl
     * curl -i https://lust.scathach.id/pornhub/random
     * 
     * @apiExample {js} JS/TS
     * import axios from "axios"
     * 
     * axios.get("https://lust.scathach.id/pornhub/random")
     * .then(res => console.log(res.data))
     * .catch(err => console.error(err))
     * 
     * @apiExample {python} Python
     * import aiohttp
     * async with aiohttp.ClientSession() as session:
     *  async with session.get("https://lust.scathach.id/pornhub/random") as resp:
     *    print(await resp.json())
     * 
     */
    const url = `${c.PORNHUB}/video/random`;
    const data = await scrapeContent(url);
    logger.info({
      path: req.path,
      query: req.query,
      method: req.method,
      ip: req.ip,
      useragent: req.get("User-Agent")
    });
    return res.json(data);
  } catch (err) {
    const e = err as Error;
    res.status(400).json(maybeError(false, e.message));
  }
}
