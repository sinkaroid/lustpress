import { scrapeContent } from "../../scraper/xvideos/xvideosGetController";
import c from "../../utils/options";
import { logger } from "../../utils/logger";
import { maybeError } from "../../utils/modifier";
import { Request, Response } from "express";
import { load } from "cheerio";
import LustPress from "../../LustPress";

const lust = new LustPress();

export async function randomXvideos(req: Request, res: Response) {
  try {
    /**
     * @api {get} /xvideos/random Get random xvideos
     * @apiName Get random xvideos
     * @apiGroup xvideos
     * @apiDescription Get a random xvideos video
     * 
     * @apiSuccessExample {json} Success-Response:
     *   HTTP/1.1 200 OK
     *   HTTP/1.1 400 Bad Request
     * 
     * @apiExample {curl} curl
     * curl -i https://lust.scathach.id/xvideos/random
     * 
     * @apiExample {js} JS/TS
     * import axios from "axios"
     * 
     * axios.get("https://lust.scathach.id/xvideos/random")
     * .then(res => console.log(res.data))
     * .catch(err => console.error(err))
     * 
     * @apiExample {python} Python
     * import aiohttp
     * async with aiohttp.ClientSession() as session:
     *  async with session.get("https://lust.scathach.id/xvideos/random") as resp:
     *    print(await resp.json())
     */
    const resolve = await lust.fetchBody(c.XVIDEOS);
    const $ = load(resolve);
    const search = $("div.thumb-under")
      .find("a")
      .map((i, el) => $(el).attr("href"))
      .get();
    const filtered = search.filter((el) => el.includes("/video"));
    const filtered_ = filtered.filter((el) => !el.includes("THUMBNUM"));
    const random = Math.floor(Math.random() * filtered_.length);
    
    const url = c.XVIDEOS + filtered[random];
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
