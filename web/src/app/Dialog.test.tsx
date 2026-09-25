import { render, screen } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { useState } from "preact/hooks";
import { Dialog } from "./Dialog";

function Harness() {
  const [open,setOpen]=useState(false);
  return <><button onClick={()=>setOpen(true)}>Open</button>{open&&<Dialog title="Keyboard dialog" description="Trapped dialog" onClose={()=>setOpen(false)}><button>First action</button><button>Last action</button></Dialog>}</>;
}

describe("Dialog keyboard contract",()=>{
 it("traps Tab, closes on Escape and restores focus",async()=>{
  const user=userEvent.setup();render(<Harness/>);const opener=screen.getByRole("button",{name:"Open"});opener.focus();await user.click(opener);
  const dialog=screen.getByRole("dialog");expect(document.activeElement).toBe(dialog);
  await user.tab();expect(document.activeElement).toBe(screen.getByRole("button",{name:"Close dialog"}));
  await user.keyboard("{Escape}");expect(screen.queryByRole("dialog")).not.toBeInTheDocument();expect(document.activeElement).toBe(opener);
 });
});
